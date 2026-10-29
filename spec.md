# Arca for Agents — SPEC (rascunho v0.1)

Evolução do **Arca** (`mastrien/arca`, Rust + SQLite + clap), um gerenciador de tarefas em CLI. O Arca for Agents mantém o conceito original e acrescenta **prazo de referência**, **projetos com pastas vinculadas** e **visibilidade da atividade de agentes de IA** (via MCP e hooks), com tarefas que tanto o usuário quanto os agentes podem criar e gerenciar.

> Contexto: o Arca original foi um projeto "build to learn" para aprender Rust. Este é um projeto maior, então é aceitável refatorar o que não servir mais. Mantenha, porém, o conceito e a linguagem do domínio (carrinho / estante).

---

## 1. Conceito (não mudar)

A analogia é um supermercado. O problema que ela resolve é pegar tarefas demais e jogar no carrinho, quando o movimento certo seria tirar do carrinho e devolver à estante.

| Termo (código) | Significado |
|---|---|
| `SHELF` (estante) | Backlog que o usuário **decidiu conscientemente não priorizar agora**. |
| `CART` (carrinho) | Também é backlog, mas o que o usuário **quer manter por perto**. Não significa "em andamento". |
| `DONE` | Concluída. |

Atenção: **carrinho não é "fazendo"**. Não modele colunas como "doing". Toda a ideia é que o carrinho seja curto e a estante seja onde as coisas esperam sem culpa.

Tarefas **diárias** (`task_is_daily`) vivem normalmente no carrinho. Quando concluídas, aparecem marcadas como `daily` + `done`. Na virada do dia, deixam um registro no histórico e **continuam existindo**, voltando a pendentes (ver seção 4).

## 2. Estado atual (resumo do código existente)

- Comandos: `list`, `add`, `done`, `edit`, `delete`, `take`, `put-back`, `check`, `history`.
- `done`, `take` e `put-back` operam por **posição** (`u8`); `edit` e `delete` por **id**.
- Tabelas: `tasks` (`task_id`, `task_title`, `task_status` em `DONE|CART|SHELF`, `task_is_daily`, `created_at`, `updated_at` com trigger) e `task_history` (`task_history_id`, `task_history_title`, `task_history_is_daily`, `task_history_date`).
- Migrations em `migrations/` via `rusqlite_migration` (`from-directory`), com `include_dir`. Banco no diretório de dados do usuário (`directories`). WAL ligado; `foreign_keys` ON.
- Dependências: `rusqlite` (bundled), `rusqlite_migration`, `include_dir`, `anyhow`, `clap` (derive), `colored`, `directories`.

## 3. Requisitos novos

### 3.1 Prazo de referência
- Campo opcional `due_date` (data, sem hora).
- **É só referência**: passar do prazo não muda status, não bloqueia e não apaga nada.
- `arca add "..." --due 2026-11-05` e `arca edit <id> --due <data|none>`.
- `list` mostra de forma humana: "em 3 dias", "hoje", "passou há 2 dias". Aceitar também formatos curtos (`+3d`, `amanhã`) é desejável, mas opcional no MVP.

### 3.2 Projetos
- Tarefa pode pertencer a **no máximo um** projeto (`project_id` opcional). Tarefa sem projeto continua válida.
- Comandos: `arca project add <nome>`, `project list`, `project rename`, `project delete` (tarefas ficam sem projeto), `project link <pasta>` / `unlink`.
- Filtro: `arca list --project <nome>`. O resumo por projeto (quantas na estante, no carrinho e feitas) ajuda a ver o andamento geral.
- **Pastas vinculadas**: um projeto pode ter várias pastas. Dado um `cwd`, o projeto é o da pasta vinculada **mais específica** (prefixo mais longo). Isso é a base para o MCP saber em qual projeto uma sessão nova começou.
- Normalizar caminhos (absoluto, separadores consistentes; o autor usa Windows e Termux/Android).

### 3.3 Autoria e política dos agentes
- Cada tarefa tem `created_by`: `user` ou `agent:<nome>` (ex.: `agent:claude-code`).
- Tarefas servem aos dois casos: o que o usuário vai fazer e o que agentes vão fazer. Nem tudo é delegável nem deve ser.
- Visão padrão do `list`: **só as do usuário**. `--agents` mostra também as dos agentes; `--only-agents` só as deles.
- Política por projeto (`projects.agent_policy`):
  - `NONE`: agente só lê.
  - `OWN` (**padrão**): agente cria, edita, conclui, move e apaga apenas tarefas com o seu próprio `created_by`. Nunca toca nas do usuário.
  - `ALL`: agente pode gerenciar todas as tarefas do projeto.
  - Tarefas sem projeto seguem a política `OWN`.
- A regra é aplicada **no código do servidor MCP** (não confiar só em instruções no prompt).
- Identidade do agente: variável de ambiente `ARCA_AGENT` na configuração do MCP/hook. É declarada pelo próprio agente (adequada para uso local, **não é segurança**).

### 3.4 Atividade de agentes
Objetivo: ver quais projetos têm agente trabalhando agora, quais agentes terminaram e esperam instrução do usuário, e "última atividade há X".

- Tabela `agent_sessions` (ver seção 5). Estados: `WORKING`, `WAITING`, `ENDED`.
- **Sinal de vida vem de hooks, não do MCP.** O MCP só roda quando o agente decide chamar uma ferramenta, então não detecta "terminei". Os hooks do Claude Code chamam `arca ping`.
- Mapeamento sugerido (confirmar nomes e payloads na documentação oficial de hooks antes de implementar):
  - início de sessão → `ping --state working`
  - usuário enviou prompt → `working`
  - fim da resposta do agente (`Stop`) → `waiting`
  - notificação de pedido de permissão/entrada → `waiting`
  - fim de sessão → `ended`
- `arca ping` deve ler `session_id` e `cwd` do JSON recebido via stdin (formato dos hooks), resolver o projeto pelo `cwd` e fazer upsert da sessão. Deve ser **rápido e silencioso** (nunca poluir a saída do agente nem falhar o hook).
- "Ativo agora": estado `WORKING` com `last_seen_at` dentro de uma janela configurável (padrão: **X = 10 min**, ajustável). `WORKING` sem sinal além da janela → "possivelmente parada".
- Outros agentes (Antigravity etc.): se não houver hooks equivalentes, o MVP cai no registro via MCP (`report_activity`) e na heurística "últimos X minutos".

## 4. Virada do dia (substitui o `check` manual)

O `arca check` original existia porque o app não sabia que o dia mudou. Agora usamos a **data do sistema (fuso local)**.

- Tabela `app_meta` guarda `last_rollover_date`.
- No início de **qualquer comando** (CLI ou MCP), se `hoje != last_rollover_date`, executa o *rollover* dentro de uma transação:
  1. **Diárias com `DONE`**: inserir uma linha em `task_history` (data = `done_at`, não a data de hoje) e voltar a tarefa para `CART`, limpando `done_at`. Ela **não é apagada nem movida de lugar**.
  2. **Não diárias com `DONE`**: inserir em `task_history` e remover de `tasks` (equivale ao antigo "limpar o carrinho").
  3. Atualizar `last_rollover_date`.
- Várias viradas puladas (ex.: fim de semana sem usar) funcionam do mesmo jeito, pois o histórico usa `done_at`.
- `arca check` pode continuar existindo como alias que força o rollover, marcado como legado (decisão em aberto, ver seção 8).
- Na visão do dia, uma diária concluída hoje aparece como `daily · done`.

## 5. Esquema (nova migration; seguir a convenção de nomes já usada em `migrations/`)

```sql
CREATE TABLE projects (
  project_id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_name TEXT NOT NULL UNIQUE,
  agent_policy TEXT NOT NULL DEFAULT 'OWN'
    CHECK(agent_policy IN ('NONE','OWN','ALL')),
  created_at DATETIME DEFAULT (DATETIME('now','localtime'))
);

CREATE TABLE project_folders (
  folder_id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  folder_path TEXT NOT NULL UNIQUE
);

ALTER TABLE tasks ADD COLUMN due_date DATE;
ALTER TABLE tasks ADD COLUMN done_at DATETIME;
ALTER TABLE tasks ADD COLUMN project_id INTEGER REFERENCES projects(project_id) ON DELETE SET NULL;
ALTER TABLE tasks ADD COLUMN created_by TEXT NOT NULL DEFAULT 'user';

ALTER TABLE task_history ADD COLUMN task_history_due_date DATE;
ALTER TABLE task_history ADD COLUMN task_history_project_id INTEGER;
ALTER TABLE task_history ADD COLUMN task_history_created_by TEXT NOT NULL DEFAULT 'user';

CREATE TABLE agent_sessions (
  session_id TEXT PRIMARY KEY,
  agent TEXT NOT NULL,
  project_id INTEGER REFERENCES projects(project_id) ON DELETE SET NULL,
  cwd TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('WORKING','WAITING','ENDED')),
  started_at DATETIME DEFAULT (DATETIME('now','localtime')),
  last_seen_at DATETIME DEFAULT (DATETIME('now','localtime'))
);

CREATE TABLE app_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

Observações de implementação:
- O trigger de `updated_at` existente deve continuar funcionando; `done_at` é gerenciado explicitamente pelos comandos (setar ao concluir, limpar ao reabrir).
- Cada conexão (CLI, hook, MCP) precisa de `PRAGMA foreign_keys=ON`, `journal_mode=WAL` e um `busy_timeout` (ex.: 5000 ms), pois vários processos escrevem no mesmo banco.
- `history` deve passar a mostrar projeto, autor e prazo quando existirem.

## 6. Servidor MCP

- Subcomando do mesmo binário: `arca mcp` (transporte **stdio**). Avaliar o SDK oficial de Rust (`rmcp`) antes de decidir entre ele e implementar o protocolo à mão.
- Ferramentas (todas operam por **id**, nunca por posição, e passam pela checagem de política):
  - `project_for_cwd(cwd)` → projeto, política e resumo de tarefas.
  - `list_tasks(project?, status?, include_user_tasks?)`
  - `add_task(title, project?, due?, status?, daily?)` (grava `created_by = ARCA_AGENT`)
  - `edit_task(id, ...)`, `move_task(id, to: CART|SHELF)`, `done_task(id)`, `delete_task(id)`
  - `report_activity(state)` (alternativa ao hook para agentes sem hooks)
- Violação de política devolve erro claro ("tarefa criada pelo usuário; política OWN"), nunca falha silenciosa.
- Caso de uso real a suportar: pedir ao agente "anote isso para fazermos depois", em vez de gravar um Markdown solto, e ver depois na estante/backlog.

## 7. Interface

- Mantém a CLI com `clap`. Comandos por posição (`done`, `take`, `put-back`) continuam para uso humano; os novos aceitam `--id`.
- `arca board`: visão tipo Trello com três colunas — **Estante | Carrinho | Feito (hoje)** — mais um painel de sessões de agentes por projeto ("trabalhando", "esperando você", "última atividade há 5h"). Começar com saída simples no terminal; TUI (ex.: `ratatui`) pode vir depois.
- `list` mostra prazo humanizado e marca tarefas de agentes quando `--agents` está ativo.
- Fica para o futuro, fora do MVP: interface gráfica/web, notificações, múltiplos usuários.

## 8. Decisões em aberto

1. `arca check` fica como alias do rollover ou é removido?
2. Tarefas não diárias concluídas: apagar na virada (como hoje) ou manter visíveis por N dias?
3. Janela padrão de "ativo agora" (proposta: 10 min) e se deve ser configurável por projeto.
4. Agentes sem hooks (Antigravity etc.): suporte além de `report_activity` via MCP?
5. Um agente pode **sugerir** concluir/mover uma tarefa do usuário (proposta pendente que o usuário aprova) em vez de ser bloqueado?
6. Quando uma sessão de agente está trabalhando numa tarefa específica, vale registrar essa ligação (tarefa ↔ sessão)?

## 9. Plano em fases (MVP primeiro)

1. **Migration + prazo**: novas colunas, `--due`, exibição humanizada.
2. **Rollover por data do sistema** + ajuste do `history`.
3. **Projetos e pastas**: comandos `project`, filtro `--project`, resolução por `cwd`.
4. **Autoria**: `created_by` e visões `--agents` / `--only-agents`.
5. **Sessões**: `arca ping` (stdin JSON), tabela `agent_sessions`, painel de atividade, configuração dos hooks do Claude Code.
6. **MCP**: `arca mcp` com ferramentas e política por projeto.
7. **Board**: `arca board` com colunas e painel de sessões.

## 10. Critérios de aceite do MVP (fases 1 a 5)

- Adicionar uma tarefa com prazo e vê-la como "em N dias" / "passou há N dias" sem mudança de status.
- Vincular uma pasta a um projeto e, a partir de um `cwd` dentro dela, resolver o projeto correto.
- Uma diária concluída hoje aparece como `daily · done`; no dia seguinte volta pendente e há uma linha no histórico com a data em que foi concluída.
- Rodando os hooks, o painel mostra um projeto como "trabalhando" durante uma sessão, "esperando você" ao fim da resposta e "última atividade há X" depois.
- Todos os comandos antigos continuam funcionando.