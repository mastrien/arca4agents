# Arca for Agents

Gerenciador de tarefas em CLI (Rust + SQLite) baseado no [Arca](https://github.com/mastrien/arca),
com prazo de referência, projetos com pastas vinculadas e visibilidade da atividade de agentes de IA.
Especificação completa em [`spec.md`](spec.md).

- **Estante** (`SHELF`): o que você decidiu conscientemente não priorizar agora.
- **Carrinho** (`CART`): o que você quer manter por perto. Não significa "fazendo".
- **Feito** (`DONE`): concluída. Na virada do dia vai para o histórico; diárias voltam a pendentes.

## Instalação

```sh
cargo install --path .
```

O banco fica no diretório de dados do usuário (no Windows, `%APPDATA%\mastrien\arca\data\arca.db`).
A variável `ARCA_DB` aponta para outro arquivo, o que é útil para testes.
Bancos da v1 são migrados automaticamente no primeiro comando.

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
| `SessionStart`, `UserPromptSubmit` | `WORKING` |
| `Stop`, `Notification` | `WAITING` |
| `SessionEnd` | `ENDED` |

```sh
arca agents                       # sessões por projeto
arca agents --window 30 --all
arca config active-window 15      # minutos até "trabalhando" virar "possivelmente parada" (padrão 10)
arca ping --state working --agent antigravity   # registro manual, para agentes sem hooks
```

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

## Desenvolvimento

```sh
cargo test
cargo clippy --all-targets
```
