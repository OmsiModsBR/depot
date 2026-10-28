# AGENTS.md

Instruções para agentes de IA (Claude Code, GitHub Copilot, Codex e afins) que trabalham neste repositório.

## Skills

Além das skills já vinculadas ao seu ambiente (por exemplo, as skills instaladas no Claude Code), use também as skills do repositório em `.ai/skills/`.

- Cada skill fica em `.ai/skills/<nome>/SKILL.md`. O cabeçalho (frontmatter) traz `name` e `description`; use a `description` para decidir quando a skill se aplica.
- Antes de começar uma tarefa, verifique se alguma skill de `.ai/skills/` cobre o que foi pedido. Se cobrir, leia o `SKILL.md` inteiro e siga as instruções dele.
- Quando o usuário invocar uma skill pelo nome (por exemplo, `/speckit-plan`), procure-a primeiro entre as skills do seu ambiente e depois em `.ai/skills/<nome>/SKILL.md`.
- Se uma skill do ambiente e uma skill de `.ai/skills/` tiverem o mesmo nome, prefira a de `.ai/skills/`, que é a versão mantida para este projeto.

Skills disponíveis hoje em `.ai/skills/`:

| Skill | Uso |
| --- | --- |
| `speckit-constitution` | Criar ou atualizar a constituição do projeto |
| `speckit-specify` | Criar a especificação de uma funcionalidade |
| `speckit-clarify` | Esclarecer pontos em aberto da especificação |
| `speckit-checklist` | Gerar checklists de qualidade dos requisitos |
| `speckit-plan` | Gerar o plano de implementação |
| `speckit-tasks` | Quebrar o plano em tarefas |
| `speckit-analyze` | Verificar consistência entre spec, plano e tarefas |
| `speckit-converge` | Reconciliar spec, plano e tarefas após mudanças |
| `speckit-implement` | Executar as tarefas |
| `speckit-taskstoissues` | Converter tarefas em issues do GitHub |

## Spec Kit

Os arquivos do Spec Kit ficam em `.ai/.specify/`, e não em `.specify/` na raiz:

- `.ai/.specify/memory/constitution.md`: constituição do projeto.
- `.ai/.specify/templates/`: templates de spec, plano, tarefas, checklist e constituição.
- `.ai/.specify/scripts/powershell/`: scripts auxiliares (`check-prerequisites.ps1`, `setup-plan.ps1`, `setup-tasks.ps1`, `resolve-template.ps1`, `create-new-feature.ps1`).
- `.ai/.specify/init-options.json`, `.ai/.specify/integration.json`: configuração.
- `.ai/.specify/feature.json`: ponteiro local para a funcionalidade ativa (ignorado pelo git).

Execute os scripts a partir da raiz do repositório. Como não existe `.specify/` na raiz, os scripts tratam `.ai/` como raiz do Spec Kit: caminhos relativos em `feature.json` e em `SPECIFY_FEATURE_DIRECTORY` são resolvidos a partir de `.ai/`, e as especificações ficam em `.ai/specs/`. Prefira caminhos absolutos ao gravar `feature.json`.

## Regras do projeto

As regras de contribuição (branches, commits, formatação, testes) estão em `CONTRIBUTING.md`. Siga-as em qualquer alteração.
