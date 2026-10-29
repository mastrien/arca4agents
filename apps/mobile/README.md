# Arca for Agents: app do celular (Flutter)

Carrinho, estante, feitas e agentes no celular, com banco próprio (funciona offline) e
sincronização com o PC na rede de casa.

- A lógica é o mesmo `arca-core` da CLI e do app desktop, chamado via
  [flutter_rust_bridge](https://cjycode.com/flutter_rust_bridge/) (`rust/src/api/arca.rs`).
  Estruturas compostas viajam como JSON (`lib/models.dart`).
- **Gestos:** deslizar para a direita avança (estante → carrinho → feita); para a esquerda, volta.
  Tocar abre a edição.
- **Sincronização:** pareie lendo o QR code do app desktop (ou de `arca serve`). Depois, o app
  sincroniza ao abrir, ao voltar para primeiro plano, a cada minuto e no "puxar para atualizar".
  Fora de casa, a falha é silenciosa.

## Desenvolvimento

Requisitos: Flutter, Android SDK + NDK num caminho **sem espaços** (aqui, `C:\Android\Sdk`) e os
alvos Rust do Android (`rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`).

```sh
flutter_rust_bridge_codegen generate   # depois de mudar rust/src/api
flutter run                            # celular conectado (USB/Wi-Fi) ou `-d windows`
flutter build apk --release
```
