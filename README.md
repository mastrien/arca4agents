# Arca for Agents

Gerenciador de tarefas em CLI (Rust + SQLite) baseado no [Arca](https://github.com/mastrien/arca),
com prazo de referência, projetos com pastas vinculadas e visibilidade da atividade de agentes de IA.
Especificação completa em [`spec.md`](spec.md); apps e sincronização em [`spec-apps.md`](spec-apps.md).

- **Estante** (`SHELF`): o que você decidiu conscientemente não priorizar agora.
- **Carrinho** (`CART`): o que você quer manter por perto. Não significa "fazendo".
- **Feito** (`DONE`): concluída. Na virada do dia vai para o histórico; diárias voltam a pendentes.

## Instalação

```sh
cargo install --path crates/arca-cli
```

O banco fica no diretório de dados do usuário (no Windows, `%APPDATA%\mastrien\arca\data\arca.db`).
A variável `ARCA_DB` aponta para outro arquivo, o que é útil para testes.
Bancos da v1 são migrados automaticamente no primeiro comando.

Para inspecionar o banco, outras ferramentas (ex.: `sqlite3`, DB Browser) servem só para
**ler**: as escritas disparam triggers de sincronização que chamam funções registradas pelo
`arca-core` (`arca_clock`, `arca_uuid`), e essas funções não existem fora dele.

## Tarefas

```sh
arca add "Escrever proposta" --due 2026-11-05 -p mestrado   # prazo: AAAA-MM-DD, hoje, amanhã, +3d, +2w
arca add "Ler 10 páginas" --daily
arca add "Ideia pra depois" --shelf
arca list                         # Cart e Shelf, com posições [n]
arca list --id -p mestrado        # ids; só um projeto (com resumo)
arca list --agents                # inclui tarefas criadas por agentes (sempre com ids)
arca list --only-agents
arca done 0                       # posição no carrinho
arca done --id 12
arca take 0 -p mestrado           # posição contada dentro da lista do projeto
arca put-back 2
arca edit 12 --due none -p none   # limpa prazo e projeto
arca delete 12
arca history
```

O prazo é **só referência**: passar dele não muda status nem apaga nada. O `list` mostra "em 3 dias", "hoje", "passou há 2 dias".

### Virada do dia

Não precisa mais de `arca check`. No primeiro comando de cada dia (inclusive o `ping` dos hooks), o Arca:

1. registra no histórico cada tarefa concluída **com a data em que foi concluída**;
2. devolve as diárias concluídas ao carrinho como pendentes;
3. remove as não diárias concluídas.

`arca check` continua existindo como comando legado: arquiva **agora** tudo que está concluído, inclusive o que foi concluído hoje.

## Projetos

```sh
arca project add mestrado                 # política de agentes padrão: OWN
arca project add lab --policy all         # none | own | all
arca project link mestrado                # vincula a pasta atual
arca project link mestrado C:\pesquisa\mestrado
arca project which                        # projeto da pasta atual (pasta vinculada mais específica)
arca project list                         # pastas e contagem: estante · carrinho · feitas
arca project policy mestrado none
arca project rename mestrado dissertacao
arca project unlink C:\pesquisa\mestrado
arca project delete dissertacao           # as tarefas ficam sem projeto
```

Os caminhos são normalizados: absolutos, com `/`, sem barra final. No Windows também valem
o formato do Git Bash (`/c/pasta`) e a comparação ignora maiúsculas. No Termux, symlinks são resolvidos.

## Atividade de agentes

`arca ping` registra o sinal de vida de uma sessão. Ele lê o JSON do hook no stdin
(`session_id`, `cwd`, `hook_event_name`), resolve o projeto pelo `cwd` e **nunca** escreve no
stdout nem falha (erros vão para `ping-errors.log`, ao lado do banco). Isso importa porque o
stdout dos hooks `SessionStart` e `UserPromptSubmit` entra no contexto do agente.

| Evento do hook | Estado |
|---|---|
| `SessionStart`, `UserPromptSubmit`, `PostToolUse` | `WORKING` |
| `Stop`, `Notification` | `WAITING` |
| `SessionEnd` | `ENDED` |

```sh
arca agents                       # sessões por projeto
arca agents --window 30 --all
arca config active-window 15      # minutos até "trabalhando" virar "possivelmente parada" (padrão 10)
arca project window mestrado 30   # janela própria do projeto ("none" volta para a global)
arca ping --state working --agent antigravity --task 12   # registro manual, ligado à tarefa #12
```

Quando a sessão está ligada a uma tarefa, o painel mostra "em #12 Título".

Sessão sem projeto ganha uma dica de origem, como `app Claude · sessão lateral`, `~/desktop/notas`,
`worktree de c:/root_lab/app`, `WSL · c:/x`, `provavelmente nuvem · /home/user/repo` ou
`· pelo MCP, sem hooks`. Ao ligar uma pasta a um projeto, as sessões que já rodavam nela passam
para o projeto.

Com o Remote Control do Claude Code ligado, o `arca ping` lê o link da sessão
(`https://claude.ai/code/session_...`) no fim do transcript, nos eventos `SessionStart`,
`UserPromptSubmit`, `Stop` e `Notification`. O desktop e o celular mostram então o botão
"Abrir no Claude". O formato do transcript não é documentado; se mudar, o botão só deixa de
aparecer.

O nome do agente vem de `--agent` ou da variável `ARCA_AGENT`. Ele é declarado pelo próprio
agente: serve para uso local e **não é** um mecanismo de segurança.

### Hooks do Claude Code

Adicione em `~/.claude/settings.json` (ou no `.claude/settings.json` de um projeto). O `arca`
precisa estar no `PATH`. No Windows, o Claude Code executa hooks no Git Bash quando ele está instalado.

```json
{
  "hooks": {
    "SessionStart": [
      { "hooks": [{ "type": "command", "command": "arca ping --agent claude-code", "timeout": 5 }] }
    ],
    "UserPromptSubmit": [
      { "hooks": [{ "type": "command", "command": "arca ping --agent claude-code", "timeout": 5 }] }
    ],
    "PostToolUse": [
      { "hooks": [{ "type": "command", "command": "arca ping --agent claude-code", "timeout": 5 }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "arca ping --agent claude-code", "timeout": 5 }] }
    ],
    "Notification": [
      {
        "matcher": "permission_prompt|idle_prompt",
        "hooks": [{ "type": "command", "command": "arca ping --agent claude-code", "timeout": 5 }]
      }
    ],
    "SessionEnd": [
      { "hooks": [{ "type": "command", "command": "arca ping --agent claude-code", "timeout": 5 }] }
    ]
  }
}
```

Se já houver uma chave `"hooks"`, junte as entradas em vez de substituí-la.

O `PostToolUse` existe porque uma pergunta ou um pedido de permissão disparam `Notification`
("esperando você"), mas a resposta não dispara `UserPromptSubmit`. Sem ele, a sessão ficaria
"esperando" enquanto o agente trabalha. O ping é síncrono (cerca de 60 ms) de propósito: com
`async`, ele poderia terminar depois do `Stop` e deixar a sessão como "trabalhando".

## Cota dos agentes

O Arca guarda o percentual usado de cada janela de limite dos agentes e mostra a série na
página de gráficos do desktop.

- **Claude Code** (planos Pro/Max): a cota chega pela statusline. O Claude Code manda o campo
  `rate_limits` (janelas de 5 h e de 7 dias) no JSON do stdin. `arca usage statusline` grava as
  janelas e imprime a linha (`5h 23% · 7d 41%`). Ele nunca falha: os erros vão para
  `ping-errors.log`. Em `~/.claude/settings.json`:

  ```json
  { "statusLine": { "type": "command", "command": "arca usage statusline" } }
  ```

- **Codex**: `arca usage codex` sobe o `codex app-server` e chama `account/rateLimits/read`.
  Enquanto está aberto, o app desktop faz essa leitura a cada 15 min. Na página de gráficos, o
  botão "Ler o Codex agora" faz uma leitura na hora.
- **Antigravity**: ainda não há fonte de cota.

Uma leitura igual à anterior só é gravada depois de 30 min, e as leituras com mais de 180 dias
são apagadas. `arca usage` mostra a última leitura de cada janela.

O celular recebe as leituras do PC na sincronização: na primeira vez, os últimos 30 dias; depois,
só as novas. Ele não lê a cota sozinho.

## Board

```sh
arca board                        # Estante | Carrinho | Feito hoje, e o painel de agentes
arca board --agents -p mestrado
arca board --width 50             # abaixo de 72 colunas (ex.: Termux) as colunas ficam empilhadas
```

## Servidor MCP

`arca mcp` é um servidor MCP (stdio) com as ferramentas `project_for_cwd`, `list_projects`,
`list_tasks`, `add_task`, `edit_task`, `move_task`, `done_task`, `delete_task`, `suggest_change`,
`list_suggestions`, `set_working_task` e `report_activity`.
Todas operam por **id** e passam pela política do projeto, aplicada no servidor:

| Política | O agente pode |
|---|---|
| `NONE` | só ler |
| `OWN` (padrão; também para tarefas sem projeto) | criar, e alterar/mover/concluir/apagar só as tarefas que ele criou |
| `ALL` | gerenciar todas as tarefas do projeto |

Uma violação devolve um erro de ferramenta com o motivo (ex.: "Tarefa #12 criada pelo usuário;
política OWN ..."). `list_tasks` informa em `can_modify` o que o agente pode alterar. Tarefas criadas
por agentes vão para a estante por padrão e ficam com `created_by = agent:<nome>`.

Registro no Claude Code:

```sh
claude mcp add --scope user arca -- arca mcp --agent claude-code
```

Toda chamada de ferramenta conta como sinal de vida. Ela vai para a sessão dos hooks do mesmo
agente na mesma pasta (ou no mesmo projeto). Se o agente não tem hooks, o servidor registra uma
sessão própria, encerrada quando ele fecha. `report_activity` informa um estado explícito, e
`set_working_task` liga a sessão à tarefa em andamento.

### Sugestões

Quando a política não deixa o agente mexer numa tarefa, ele pode propor a mudança com
`suggest_change` (concluir, mover, editar ou apagar, com um motivo). Só você aplica:

```sh
arca suggestions          # pendentes (--all inclui as resolvidas)
arca approve 3
arca reject 4
```

O `list` avisa quando há sugestões pendentes, e o `board` as mostra no fim.

## Sincronização na rede de casa

O PC roda o servidor, e os outros aparelhos (o app do celular, ou outro PC com a CLI) sincronizam
com ele. Cada aparelho tem uma cópia completa e funciona offline. Na sincronização, a edição mais
recente de cada item vence, e as exclusões também são propagadas. Detalhes em [`spec-apps.md`](spec-apps.md).

```sh
arca serve                        # no PC: mostra o QR code e o código de pareamento (porta 7377)
arca sync pair '<código>'         # no outro aparelho; também aceita @arquivo
arca sync now                     # envia e recebe
arca sync peers
arca sync unpair DESKTOP-XYZ
arca sync rotate-token            # no PC: invalida todos os pareamentos
```

Cada aparelho se apresenta pelo nome (o app do celular usa o modelo, ex.: "Pixel 8"), e o app
desktop mostra quem sincronizou e quando.

O tráfego é HTTP sem criptografia, protegido por um token. Isso basta na rede de casa, mas não
exponha a porta na internet. No primeiro `arca serve`, o Windows pede para liberar o firewall
(redes privadas).

## Apps e releases

- **Desktop (Tauri):** [`apps/desktop`](apps/desktop). **Celular (Flutter):** [`apps/mobile`](apps/mobile).
- Os instaladores saem no [GitHub Releases](https://github.com/mastrien/arca4agents/releases). Depois
  da primeira instalação, os dois apps avisam quando há versão nova e se atualizam por conta própria;
  no Android, o sistema pede uma confirmação.

Para publicar uma versão:

```sh
python scripts/version.py 0.5.0     # acerta Cargo, Tauri, package.json e pubspec
git commit -am "v0.5.0" && git tag v0.5.0 && git push && git push --tags
```

O workflow `release.yml` gera o instalador Windows assinado (com o `latest.json` do updater), a
CLI e o APK assinado, e só publica o release quando todos os arquivos subiram. Ele precisa destes
secrets do repositório: `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`,
`ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`. As chaves ficam fora do
repositório e **precisam de backup**: sem elas, os apps instalados deixam de aceitar atualizações.

## Desenvolvimento

```
crates/arca-core   núcleo (banco, migrations, rollover, política, sugestões, sync); não imprime nada
crates/arca-cli    binário `arca` (CLI, MCP, hooks)
```

```sh
cargo test
cargo clippy --all-targets
```

**Migrations só acrescentam:** tabela, coluna com default ou que aceita NULL, índice. Nunca
renomeiam nem apagam o que existe. A partir da 0.8.0, uma versão anterior abre um banco migrado
por uma mais nova em vez de recusar. Isso importa porque a CLI dos hooks (`cargo install`), o app
desktop e o celular não se atualizam ao mesmo tempo. Para testar o app em modo dev com migration
nova, use `ARCA_DB` com um banco separado.
