# Arca for Agents: app desktop (Tauri)

Board (Estante | Carrinho | Feito hoje), painel de agentes, sugestões, projetos, histórico e o
servidor de sincronização para o celular. Usa o mesmo banco da CLI, então hooks, MCP e app veem as
mesmas tarefas. O board é relido a cada 3 s.

```sh
npm install
npm run tauri dev      # desenvolvimento
npm run tauri build    # instalador em ../../target/release/bundle
```

`ARCA_DB=<arquivo>` abre outro banco (útil para testes). O servidor de sync fica ligado por padrão
(porta 7377); ele pode ser desligado na janela "Sincronização", e a escolha fica salva no banco.

O backend (`src-tauri/src/lib.rs`) só expõe comandos finos sobre `arca_core::api`; a lógica fica no núcleo.
