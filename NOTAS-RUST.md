# Notas de Rust: o que mudou em relação ao Arca v1

Pontos do código original que foram corrigidos na refatoração, para ler com calma.
Os exemplos "depois" estão no código atual.

## Bugs de verdade

1. **`list -s` e `list -c` sempre mostravam tudo.** O `main` passava `Some(shelf)` e `Some(cart)`, então
   `cart.is_none()` era sempre `false` e as duas condições davam `true`. Quando um `Option` nunca é `None`,
   ele só esconde a lógica: use `bool` direto e um `match (cart, shelf)` (veja `lib.rs::list`).
2. **`arca done 99` dava panic.** `vec[position]` entra em pânico fora dos limites. Use `vec.get(i)`, que
   devolve `Option`, e transforme em erro com `.with_context(|| ...)` (veja `lib.rs::resolve`).
3. **`render_shelf` imprimia "Cart".** Era um copiar/colar sem ajustar o título.
4. **`check` não era transacional.** Se o processo morresse no meio do loop, o histórico podia ganhar
   linhas de tarefas que continuavam no carrinho, e o próximo `check` as registraria de novo. Operações
   que precisam acontecer juntas vão numa transação (veja `persistence.rs::archive_done`).
5. **Os pragmas eram aplicados depois das migrations**, e não havia `busy_timeout`. Com vários processos
   escrevendo no banco (CLI + hooks), um deles recebe `SQLITE_BUSY` em vez de esperar.

## Tratamento de erros

- **`.unwrap()` em tudo.** Cada `unwrap` é um panic em potencial, com mensagem feia. Nas funções, devolva
  `anyhow::Result` e use `?`; trate o erro uma única vez, no `main`, imprimindo uma mensagem e devolvendo
  `ExitCode::FAILURE`.
- **`.context("...").unwrap()`** anula o `context`: a mensagem é montada e logo depois o programa entra em pânico.
  O `context` só serve quando o erro sobe com `?`.
- **`fn main() -> io::Result<()>`** que nunca devolve erro não acrescenta nada. Use `ExitCode` ou `anyhow::Result<()>`.
- **`Status::UNKNOWN` para valores inválidos** esconde dados corrompidos. Prefira falhar a conversão
  (`fn parse(s) -> Result<Self>`) a carregar um estado "desconhecido" pelo programa.

## Ownership e tipos

- **`fn done(db: Database, ...)`** move o banco para dentro da função. Quem só lê ou usa precisa de um
  empréstimo: `&Database`.
- **`Option<bool>` como parâmetro** e `if id.is_some() && (id.unwrap() == true)`: quando não existe o caso
  "não informado", use `bool`. Quando existe, `id == Some(true)` resolve.
- **`u8` para posição** limita a 256 tarefas e falha na leitura de `256`. Índices em Rust são `usize`.
- **`-> ()`** é implícito; não precisa escrever.
- **Variantes em CAIXA ALTA** (`Status::DONE`): a convenção é `CamelCase` (`Status::Done`), e o texto do
  banco fica num `as_str()`. O compilador avisa sobre isso (`non_camel_case_types`).

## Duplicação e fragilidade

- **O mapeamento de linha → `Task` estava copiado três vezes.** Uma função `task_from_row` e uma constante
  `TASK_SELECT` resolvem, e adicionar uma coluna passa a ser uma mudança num lugar só.
- **`SELECT *` com `row.get(0..n)`** depende da ordem das colunas na tabela. Liste as colunas explicitamente.
- **`render_cart`/`render_shelf` refiltravam o que o banco já tinha filtrado.** Filtre num lugar só.
- **`collect()` em `Result`**: em vez de `for item in iter { v.push(item?) }`, use
  `iter.collect::<rusqlite::Result<Vec<_>>>()?`.
