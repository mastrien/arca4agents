import 'dart:io';

import 'package:flutter/material.dart';
import 'package:ota_update/ota_update.dart';
import 'package:path_provider/path_provider.dart';

import 'models.dart';
import 'screens.dart';
import 'src/rust/api/arca.dart' as arca;
import 'src/rust/frb_generated.dart';
import 'store.dart';
import 'updater.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  // ARCA_DB aponta para outro banco (testes no desktop); no celular vale a pasta do app.
  final custom = Platform.environment['ARCA_DB'];
  final path = custom ?? '${(await getApplicationDocumentsDirectory()).path}/arca.db';
  await arca.openDatabase(path: path);

  final store = ArcaStore();
  await store.start();
  runApp(ArcaApp(store: store));
}

class ArcaApp extends StatelessWidget {
  final ArcaStore store;
  const ArcaApp({super.key, required this.store});

  @override
  Widget build(BuildContext context) {
    const seed = Color(0xFF2F6F5E);
    return MaterialApp(
      title: 'Arca',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(colorSchemeSeed: seed, useMaterial3: true),
      darkTheme: ThemeData(colorSchemeSeed: seed, brightness: Brightness.dark, useMaterial3: true),
      home: HomeScreen(store: store),
    );
  }
}

class HomeScreen extends StatefulWidget {
  final ArcaStore store;
  const HomeScreen({super.key, required this.store});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  int tab = 0; // 0 carrinho, 1 estante, 2 feito, 3 agentes
  AppUpdate? update;
  String? updateStatus;

  ArcaStore get store => widget.store;

  @override
  void initState() {
    super.initState();
    AppUpdate.check().then((u) => mounted ? setState(() => update = u) : null);
  }

  void installUpdate() {
    setState(() => updateStatus = 'baixando…');
    update!.install().listen(
      (event) => setState(() => updateStatus = switch (event.status) {
            OtaStatus.DOWNLOADING => 'baixando ${event.value ?? ''}%',
            OtaStatus.INSTALLING || OtaStatus.INSTALLATION_DONE => 'abrindo o instalador…',
            OtaStatus.CHECKSUM_ERROR => 'o arquivo baixado não confere; tente de novo',
            _ => 'falhou (${event.status.name}); tente de novo',
          }),
      onError: (e) => setState(() => updateStatus = 'falhou: $e'),
    );
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: store,
      builder: (context, _) {
        final board = store.board;
        final pending = board?.suggestions.length ?? 0;
        return Scaffold(
          appBar: AppBar(
            title: Text(store.project ?? 'Arca'),
            actions: [
              if (board != null) _projectMenu(board),
              IconButton(
                tooltip: store.lastSyncMessage ?? 'Sincronizar',
                onPressed: store.peers.isEmpty
                    ? () => openSync(context, store)
                    : store.syncing
                        ? null
                        : () => store.sync(),
                icon: store.syncing
                    ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
                    : Icon(store.peers.isEmpty ? Icons.sync_disabled : Icons.sync),
              ),
              PopupMenuButton<String>(
                onSelected: (value) {
                  switch (value) {
                    case 'history':
                      openHistory(context, store);
                    case 'projects':
                      openProjects(context, store);
                    case 'sync':
                      openSync(context, store);
                    default:
                      store.setAuthors(value);
                  }
                },
                itemBuilder: (_) => [
                  CheckedPopupMenuItem(value: 'user', checked: store.authors == 'user', child: const Text('Minhas tarefas')),
                  CheckedPopupMenuItem(value: 'all', checked: store.authors == 'all', child: const Text('+ tarefas de agentes')),
                  CheckedPopupMenuItem(value: 'agents', checked: store.authors == 'agents', child: const Text('Só de agentes')),
                  const PopupMenuDivider(),
                  const PopupMenuItem(value: 'history', child: Text('Histórico')),
                  const PopupMenuItem(value: 'projects', child: Text('Projetos')),
                  const PopupMenuItem(value: 'sync', child: Text('Sincronização')),
                ],
              ),
            ],
          ),
          body: Column(
            children: [
              if (update != null)
                MaterialBanner(
                  content: Text(updateStatus ?? 'Nova versão ${update!.version} disponível.'),
                  actions: [
                    TextButton(onPressed: () => setState(() => update = null), child: const Text('Depois')),
                    FilledButton(onPressed: updateStatus == null ? installUpdate : null, child: const Text('Atualizar')),
                  ],
                ),
              if (store.error != null)
                MaterialBanner(
                  content: Text(store.error!),
                  backgroundColor: Theme.of(context).colorScheme.errorContainer,
                  actions: [TextButton(onPressed: store.clearError, child: const Text('OK'))],
                ),
              if (store.lastSyncMessage != null && tab != 3)
                Padding(
                  padding: const EdgeInsets.only(top: 4),
                  child: Text(store.lastSyncMessage!, style: Theme.of(context).textTheme.bodySmall),
                ),
              Expanded(
                child: board == null
                    ? const Center(child: CircularProgressIndicator())
                    : RefreshIndicator(
                        onRefresh: () => store.peers.isEmpty ? store.reload() : store.sync(),
                        child: switch (tab) {
                          0 => TaskList(store: store, tasks: board.cart, empty: 'Carrinho vazio.\nPegue algo da estante.', showProject: store.project == null),
                          1 => TaskList(store: store, tasks: board.shelf, empty: 'Estante vazia.', showProject: store.project == null),
                          2 => TaskList(store: store, tasks: board.done, empty: 'Nada concluído hoje.', showProject: store.project == null),
                          _ => AgentsPanel(store: store, board: board),
                        },
                      ),
              ),
            ],
          ),
          floatingActionButton: tab == 3
              ? null
              : FloatingActionButton(
                  onPressed: () => showTaskSheet(context, store, defaultStatus: tab == 1 ? 'SHELF' : 'CART'),
                  child: const Icon(Icons.add),
                ),
          bottomNavigationBar: NavigationBar(
            selectedIndex: tab,
            onDestinationSelected: (i) => setState(() => tab = i),
            destinations: [
              NavigationDestination(icon: const Icon(Icons.shopping_cart_outlined), label: 'Carrinho ${board?.cart.length ?? ''}'),
              NavigationDestination(icon: const Icon(Icons.shelves), label: 'Estante ${board?.shelf.length ?? ''}'),
              NavigationDestination(icon: const Icon(Icons.check_circle_outline), label: 'Feito ${board?.done.length ?? ''}'),
              NavigationDestination(
                icon: Badge(isLabelVisible: pending > 0, label: Text('$pending'), child: const Icon(Icons.smart_toy_outlined)),
                label: 'Agentes',
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _projectMenu(Board board) {
    return PopupMenuButton<String?>(
      tooltip: 'Projeto',
      icon: const Icon(Icons.folder_outlined),
      onSelected: (value) => store.setProject(value == '' ? null : value),
      itemBuilder: (_) => [
        CheckedPopupMenuItem(value: '', checked: store.project == null, child: const Text('Todos os projetos')),
        for (final p in board.projects) CheckedPopupMenuItem(value: p.name, checked: store.project == p.name, child: Text(p.name)),
      ],
    );
  }
}
