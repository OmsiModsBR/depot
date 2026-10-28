//! Talking to an installed OmsiLaunch: `--json` command lines for the facts Depot needs
//! (version, plan, recovery, modules) and the start of a session. Nothing here knows how
//! OmsiLaunch works inside; it only uses the documented command line and exit codes.

use super::errors::{explain, extract_code, shim_problem, Fix};
use super::runner::{OriginalEngineRunner, RunOutput};
use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Child;

pub struct Client<'a, R: OriginalEngineRunner> {
    pub runner: &'a R,
    /// `OmsiLaunch.exe` (console) and `OmsiLaunchW.exe` (no console, for the session).
    pub exe: PathBuf,
    pub windowed: PathBuf,
}

impl<'a, R: OriginalEngineRunner> Client<'a, R> {
    pub fn in_root(runner: &'a R, root: &Path) -> Self {
        Client {
            runner,
            exe: root.join("OmsiLaunch.exe"),
            windowed: root.join("OmsiLaunchW.exe"),
        }
    }

    fn call(&self, args: &[&str]) -> Result<RunOutput> {
        let mut a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        a.push("--json".into());
        self.runner
            .run(&self.exe, &a)
            .map_err(|e| anyhow!("OmsiLaunch could not be run: {e}"))
    }
}

/// The payload of an answer: the `result` of an envelope, else the object itself.
pub fn payload(stdout: &str) -> Option<Value> {
    let v: Value = serde_json::from_str(stdout.trim()).ok()?;
    match v.get("result") {
        Some(r) if v.get("ok").is_some() => Some(r.clone()),
        _ => Some(v),
    }
}

fn str_of(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionInfo {
    pub version: String,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub code: String,
    pub message: &'static str,
    pub fix: Fix,
}

impl Serialize for Fix {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{self:?}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub runnable: bool,
    pub problems: Vec<Problem>,
}

fn problem(code: &str) -> Problem {
    let (message, fix) = explain(code);
    Problem {
        code: code.to_string(),
        message,
        fix,
    }
}

/// Problems named by a run that failed: codes in the plan, in the output or in the error text.
pub fn problems_of(out: &RunOutput) -> Vec<Problem> {
    let mut codes: Vec<String> = Vec::new();
    if let Some(v) = payload(&out.stdout) {
        let diags = v
            .get("Diagnostics")
            .or_else(|| v.get("diagnostics"))
            .and_then(Value::as_array);
        for d in diags.into_iter().flatten() {
            let code = d
                .get("Code")
                .or_else(|| d.get("code"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| d.as_str().and_then(extract_code));
            if let Some(c) = code.filter(|c| c.starts_with("OL_E_")) {
                codes.push(c);
            }
        }
        if let Some(c) = v
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(Value::as_str)
        {
            codes.push(c.to_string());
        }
    }
    for text in [&out.stderr, &out.stdout] {
        if let Some(c) = extract_code(text) {
            codes.push(c);
        }
    }
    codes.dedup();
    let mut seen = std::collections::BTreeSet::new();
    codes.retain(|c| seen.insert(c.clone()));
    let mut list: Vec<Problem> = codes.iter().map(|c| problem(c)).collect();
    if list.is_empty() {
        if let Some((message, fix)) = out.code.and_then(shim_problem) {
            list.push(Problem {
                code: format!("SHIM_{}", out.code.unwrap_or(0)),
                message,
                fix,
            });
        }
    }
    list
}

impl<R: OriginalEngineRunner> Client<'_, R> {
    pub fn version(&self) -> Result<VersionInfo> {
        let out = self.call(&["/version"])?;
        if out.code != Some(0) {
            let p = problems_of(&out);
            return Err(anyhow!(p
                .first()
                .map(|p| p.message)
                .unwrap_or("OmsiLaunch did not answer")));
        }
        let v = payload(&out.stdout)
            .ok_or_else(|| anyhow!("OmsiLaunch answered with something unreadable"))?;
        Ok(VersionInfo {
            version: str_of(&v, "version").ok_or_else(|| anyhow!("no version in the answer"))?,
            protocol: str_of(&v, "protocol_version").unwrap_or_default(),
        })
    }

    /// Plans a session without starting it (`args` are the launch arguments).
    pub fn plan(&self, args: &[String]) -> Result<Plan> {
        let mut a: Vec<&str> = args.iter().map(String::as_str).collect();
        a.push("/plan");
        let out = self.call(&a)?;
        let runnable = out.code == Some(0)
            && payload(&out.stdout)
                .and_then(|v| v.get("IsRunnable").and_then(Value::as_bool))
                .unwrap_or(false);
        Ok(Plan {
            runnable,
            problems: if runnable { vec![] } else { problems_of(&out) },
        })
    }

    pub fn recovery_status(&self) -> Result<Recovery> {
        self.recovery("/recovery-status")
    }

    pub fn recover(&self) -> Result<Recovery> {
        self.recovery("/recover")
    }

    fn recovery(&self, cmd: &str) -> Result<Recovery> {
        let out = self.call(&[cmd])?;
        if out.code == Some(7) {
            return Ok(Recovery {
                pending: false,
                recovered: false,
                busy: true,
                failed: false,
            });
        }
        // an unknown state is never reported as a clean one
        if !matches!(out.code, Some(0) | Some(8)) {
            let p = problems_of(&out);
            return Err(anyhow!(p
                .first()
                .map(|p| p.message)
                .unwrap_or("OmsiLaunch did not answer")));
        }
        let v = payload(&out.stdout)
            .filter(Value::is_object)
            .ok_or_else(|| anyhow!("OmsiLaunch answered with something unreadable"))?;
        let b = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
        Ok(Recovery {
            pending: b("pending"),
            recovered: b("recovered"),
            busy: false,
            failed: out.code == Some(8),
        })
    }

    pub fn modules(&self) -> Result<Vec<ModuleInfo>> {
        let out = self.call(&["/list:RuntimeModules"])?;
        if out.code != Some(0) {
            return Ok(vec![]);
        }
        let v = payload(&out.stdout).unwrap_or(Value::Null);
        let list = v
            .get("modules")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(list
            .iter()
            .filter_map(|m| {
                Some(ModuleInfo {
                    id: str_of(m, "id")?,
                    version: str_of(m, "version").unwrap_or_default(),
                    activation: str_of(m, "activation").unwrap_or_default(),
                    valid: m.get("valid").and_then(Value::as_bool).unwrap_or(true),
                })
            })
            .collect())
    }

    /// Starts the session with `OmsiLaunchW.exe`; the process runs until the game ends.
    pub fn start(&self, args: &[String]) -> std::io::Result<Child> {
        self.runner.spawn(&self.windowed, args)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recovery {
    pub pending: bool,
    pub recovered: bool,
    /// The installation is held by a running session.
    pub busy: bool,
    pub failed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleInfo {
    pub id: String,
    pub version: String,
    pub activation: String,
    pub valid: bool,
}
