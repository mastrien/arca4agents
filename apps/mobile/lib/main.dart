import 'dart:io';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter/material.dart';
import 'package:ota_update/ota_update.dart';
import 'package:path_provider/path_provider.dart';

import 'screens.dart';
import 'src/rust/api/arca.dart' as arca;
import 'src/rust/frb_generated.dart';
import 'store.dart';
import 'theme.dart';
import 'updater.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  // ARCA_DB aponta para outro banco (testes no desktop); no celular vale a pasta do app.
  final custom = Platform.environment['ARCA_DB'];
  final path = custom ?? '${(await getApplicationDocumentsDirectory()).path}/arca.db';
  await arca.openDatabase(path: path);
  // O PC mostra o celular pelo modelo ("Pixel 8") na lista de aparelhos sincronizados.
  if (Platform.isAndroid) {
    try {
      await arca.setDeviceName(name: (await DeviceInfoPlugin().androidInfo).model);
    } catch (_) {}
  }

  final store = ArcaStore();
  await store.start();
  runApp(ArcaApp(store: store));
}

class ArcaApp extends StatelessWidget {
  final ArcaStore store;
  const ArcaApp({super.key, required this.store});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Arca4agents',
      debugShowCheckedModeBanner: false,
      theme: arcaTheme(Brightness.light),
      darkTheme: arcaTheme(Brightness.dark),
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
  int section = 0; // 0 tarefas, 1 agentes, 2 mais
  String list = 'CART';
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
    final c = context.arca;
    return ListenableBuilder(
      listenable: store,
      builder: (context, _) {
        final board = store.board;
        final attention = board == null ? 0 : board.suggestions.length + board.sessions.where((s) => s.activity == 'waiting').length;
        return Scaffold(
          body: SafeArea(
            bottom: false,
            child: Column(
              children: [
                if (update != null)
                  Notice(
                    text: updateStatus ?? 'Nova versão ${update!.version} disponível.',
                    actions: [
                      TextButton(onPressed: () => setState(() => update = null), child: const Text('Depois')),
                      TextButton(onPressed: updateStatus == null ? installUpdate : null, child: Text('Atualizar', style: TextStyle(color: c.accent, fontWeight: FontWeight.w600))),
                    ],
                  ),
                if (store.error != null)
                  Notice(
                    text: store.error!,
                    warning: true,
                    actions: [TextButton(onPressed: store.clearError, child: const Text('OK'))],
                  ),
                Expanded(
                  child: board == null
                      ? const Center(child: CircularProgressIndicator())
                      : switch (section) {
                          0 => TasksScreen(
                              store: store,
                              board: board,
                              list: list,
                              onList: (v) => setState(() => list = v),
                              onAttention: () => setState(() => section = 1),
                            ),
                          1 => AgentsScreen(store: store, board: board),
                          _ => MoreScreen(store: store),
                        },
                ),
              ],
            ),
          ),
          floatingActionButton: section == 0 && board != null
              ? FloatingActionButton(
                  tooltip: 'Nova tarefa',
                  onPressed: () => showTaskSheet(context, store),
                  child: const Icon(Icons.add, size: 26),
                )
              : null,
          bottomNavigationBar: NavigationBar(
            selectedIndex: section,
            onDestinationSelected: (i) => setState(() => section = i),
            height: 68,
            destinations: [
              const NavigationDestination(icon: Icon(Icons.format_list_bulleted), label: 'Tarefas'),
              NavigationDestination(
                icon: Badge(
                  isLabelVisible: attention > 0,
                  backgroundColor: c.attentionDot,
                  label: Text('$attention'),
                  child: const Icon(Icons.smart_toy_outlined),
                ),
                label: 'Agentes',
              ),
              const NavigationDestination(icon: Icon(Icons.more_horiz), label: 'Mais'),
            ],
          ),
        );
      },
    );
  }
}

/// Faixa de aviso no topo (atualização, erro).
class Notice extends StatelessWidget {
  final String text;
  final bool warning;
  final List<Widget> actions;
  const Notice({super.key, required this.text, this.warning = false, this.actions = const []});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Container(
      width: double.infinity,
      margin: const EdgeInsets.fromLTRB(16, 8, 16, 0),
      padding: const EdgeInsets.fromLTRB(14, 6, 6, 6),
      decoration: BoxDecoration(color: warning ? c.attentionTint : c.accentTint, borderRadius: BorderRadius.circular(12)),
      child: Row(children: [
        Expanded(child: Text(text, style: TextStyle(fontSize: 13, color: warning ? c.attention : c.accent))),
        ...actions,
      ]),
    );
  }
}
