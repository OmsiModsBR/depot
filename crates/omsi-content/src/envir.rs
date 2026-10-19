//! `envir.cfg` - sky textures, twilight, light colours (unit `mc_himmel`).

use omsi_cfg::CfgFile;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CloudType {
    pub name: String,
    pub texture: String,
    pub height: f32,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Envir {
    pub sky_textures: [String; 3],
    pub twilight_start_end: (f32, f32),
    /// 5 sun altitudes × RGB for direct sun light
    pub light_color_a: [[f32; 3]; 5],
    pub light_color_b: [[f32; 3]; 5],
    pub light_color_c: [[f32; 3]; 5],
    pub cloud_types: Vec<CloudType>,
}

fn colors(r: &mut omsi_cfg::CfgReader) -> [[f32; 3]; 5] {
    let mut out = [[0.0; 3]; 5];
    for c in out.iter_mut() {
        *c = r.f32s::<3>();
    }
    out
}

impl Envir {
    pub fn load(path: &Path) -> Result<Envir, omsi_cfg::CfgError> {
        let f = CfgFile::read(path)?;
        Ok(Self::parse(&f))
    }
    pub fn parse(f: &CfgFile) -> Envir {
        let mut e = Envir {
            twilight_start_end: (-18.0, 10.0),
            ..Default::default()
        };
        let mut r = f.reader().disabled_blocks();
        while let Some(k) = r.next_keyword() {
            match k.as_str() {
                "sky_textures" => {
                    e.sky_textures = [
                        r.str().to_string(),
                        r.str().to_string(),
                        r.str().to_string(),
                    ]
                }
                "twilight_start_end" => e.twilight_start_end = (r.f32(), r.f32()),
                "lightcolor_a" => e.light_color_a = colors(&mut r),
                "lightcolor_b" => e.light_color_b = colors(&mut r),
                "lightcolor_c" => e.light_color_c = colors(&mut r),
                "cloudtype" => e.cloud_types.push(CloudType {
                    name: r.str().to_string(),
                    texture: r.str().to_string(),
                    height: r.f32(),
                    code: r.str().to_string(),
                }),
                _ => {}
            }
        }
        e
    }

    /// The cloud type of that name (`Weather/clouds.cfg` lists them with the same
    /// `[cloudtype]` blocks), ignoring case and surrounding blanks as OMSI does.
    pub fn cloud_type(&self, name: &str) -> Option<&CloudType> {
        let name = name.trim();
        self.cloud_types
            .iter()
            .find(|t| t.name.trim().eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_types_are_found_by_name() {
        let f = CfgFile::from_bytes(
            Path::new("clouds.cfg"),
            b"[cloudtype]\r\nCumulus 1\r\ncu1.dds\r\n2000\r\nsct\r\n\r\n\
              [cloudtype]\r\nOvercast\r\novc.dds\r\n800\r\novc\r\n",
        );
        let e = Envir::parse(&f);
        assert_eq!(e.cloud_types.len(), 2);
        let ovc = e.cloud_type("overcast").unwrap();
        assert_eq!(ovc.texture, "ovc.dds");
        assert_eq!(ovc.height, 800.0);
        assert_eq!(ovc.code, "ovc");
        assert_eq!(e.cloud_type(" Cumulus 1 ").unwrap().texture, "cu1.dds");
        assert!(e.cloud_type("Cumulus 2").is_none());
    }
}
