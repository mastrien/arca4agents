# Arca for Agents: apps e sincronização (rascunho v0.1)

Continuação do [`spec.md`](spec.md), cujas fases 1 a 7 e decisões estão concluídas.
Objetivo: um app desktop (Tauri) e um app de celular (Flutter), com atualização automática,
que sincronizam os bancos na rede de casa.

## 1. Decisões já tomadas (2026-10-29)

- **Um único núcleo em Rust.** CLI, Tauri e Flutter usam o mesmo crate (`arca-core`). O Flutter
  o acessa via [`flutter_rust_bridge`](https://github.com/fzyzcjy/flutter_rust_bridge). Rollover,
  política e sync não são reimplementados em Dart.
- **Sincronização só na rede de casa.** O computador é o servidor; o celular sincroniza quando
  está na mesma rede.
- **Distribuição pelo GitHub Releases** de um repositório novo e público (`mastrien/arca4agents`).
  O `mastrien/arca` continua sendo só o código original do autor.
- **Android:** SDK e NDK instalados localmente para testar no celular; o CI também gera o APK.

## 2. Estrutura

```
crates/arca-core     núcleo: banco, migrations, rollover, projetos, política, sugestões, sync
crates/arca-cli      binário `arca`: CLI, MCP, hooks (ping), `arca serve`
apps/desktop         Tauri v2 (src-tauri usa arca-core; front em React + TypeScript + Vite)
apps/mobile          Flutter (rust/ expõe a API do arca-core via flutter_rust_bridge)
```

O `arca-core` não imprime nada: devolve dados (`serde::Serialize`), e cada interface os apresenta.

## 3. Sincronização

### 3.1 Modelo

- Cada dispositivo tem uma **réplica completa** e funciona offline.
- Linhas sincronizadas ganham um `uuid` global. Os ids inteiros (`#12`) continuam locais e só
  servem para exibição; o mesmo item pode ter ids diferentes no PC e no celular.
- Referências entre tabelas (tarefa → projeto, sugestão → tarefa) viajam como `uuid` e são
  traduzidas para o id local ao aplicar.
- Cada escrita carimba a linha com um **relógio lógico híbrido (HLC)**, um texto ordenável no
  formato `<ms>:<contador>:<dispositivo>`. O carimbo é feito por triggers SQLite que chamam uma
  função registrada pelo núcleo, para que nenhum caminho de escrita esqueça.
- Conflitos: **a última escrita vence, por linha.** Com um único usuário, editar a mesma tarefa
  nos dois aparelhos entre duas sincronizações é raro; se acontecer, vale a edição mais recente.
- Exclusões viram **tombstones** (`sync_tombstones`), que também viajam.

### 3.2 O que sincroniza

| Tabela | Sincroniza | Observação |
|---|---|---|
| `projects` | sim | |
| `tasks` | sim | |
| `task_history` | sim | O uuid é determinístico (`h:<uuid da tarefa>:<done_at>`), então os dois aparelhos fazem a virada do dia sem duplicar o histórico |
| `suggestions` | sim | Dá para aprovar ou rejeitar no celular |
| `agent_sessions` | só PC → celular | Leitura no celular, para ver o que os agentes estão fazendo |
| `project_folders` | não | Os caminhos só fazem sentido no PC |
| `app_meta` | não | Configuração local |

Depois de aplicar uma sincronização, o dispositivo arquiva o que estiver concluído antes de hoje,
para tratar a tarefa que foi concluída no outro aparelho num dia anterior.

### 3.3 Protocolo

O servidor é o `arca serve` (CLI) ou o próprio app Tauri. O protocolo é HTTP na rede local:

1. `GET /sync/changes?since=<cursor>`: o servidor devolve as linhas e os tombstones com relógio maior que o cursor, mais o novo cursor.
2. O cliente aplica: só sobrescreve uma linha local se o relógio remoto for maior.
3. `POST /sync/changes`: o cliente envia o que mudou desde o último envio, e o servidor aplica com a mesma regra.

O cliente guarda os dois cursores por servidor pareado.

### 3.4 Pareamento e segurança

- O servidor gera um token secreto e mostra um **QR code** (no terminal e no app desktop) com `{host, porta, token}`. O celular lê o QR uma vez.
- Toda requisição leva `Authorization: Bearer <token>`.
- MVP: HTTP sem TLS, aceitável dentro da rede de casa. O token pode ser revogado com um novo pareamento. TLS com certificado fixado fica para depois.
- Se o IP do PC mudar, o celular tenta o último endereço conhecido. A descoberta por mDNS (`_arca._tcp`) vem numa fase seguinte, porque no Android ela exige um multicast lock.

## 4. Atualização automática

- **Desktop:** `tauri-plugin-updater` lendo o `latest.json` do último release. Os pacotes são
  assinados com uma chave do Tauri; a privada fica num secret do repositório e em backup local.
- **Android:** sem Play Store, o app consulta o último release no GitHub, baixa o APK e abre o
  instalador do sistema. O Android exige uma confirmação do usuário; não existe atualização silenciosa.
  Todas as versões precisam ser assinadas com **o mesmo keystore**, senão o Android recusa a
  atualização. Esse keystore precisa de backup.
- **CI (GitHub Actions):** uma tag `v*` gera o instalador Windows assinado + `latest.json` e o APK
  assinado, e publica o release.

## 5. Fases

8. **Núcleo:** separar o workspace (`arca-core` + `arca-cli`) e mover a lógica sem `println!` para o núcleo, com uma API de dados.
9. **Sync no núcleo:** migration com uuid, HLC, triggers e tombstones; aplicação de mudanças; testes com dois bancos.
10. **`arca serve`** (HTTP, token, QR) e cliente de sync no núcleo.
11. **App desktop (Tauri):** board, tarefas, projetos, sugestões, painel de agentes, servidor de sync em segundo plano, pareamento.
12. **App Flutter:** as mesmas telas, com banco local, sincronização e leitura do QR.
13. **Releases e atualização automática:** repositório novo, CI, assinaturas, updater no desktop e no Android.

## 6. Em aberto

1. Notificações no celular (ex.: "agente esperando você") — fora do MVP.
2. Sincronizar também fora de casa (Tailscale etc.) — decidido que não, por enquanto.
3. Merge por campo em vez de por linha, se conflitos virarem um problema na prática.
