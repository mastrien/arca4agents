import 'dart:async';
import 'dart:convert';

import 'package:flutter/widgets.dart';

import 'models.dart';
import 'src/rust/api/arca.dart' as arca;

/// Estado do app: board atual, filtros e sincronização. Toda escrita passa pelo núcleo em Rust.
class ArcaStore extends ChangeNotifier with WidgetsBindingObserver {
  Board? board;
  String? project;
  String authors = 'all';
  String? error;

  List<Peer> peers = [];
  bool syncing = false;
  String? lastSyncMessage;
  /// Resultado da última tentativa: `null` antes da primeira.
  bool? lastSyncOk;

  Timer? _timer;

  /// Com o app aberto, tenta sincronizar a cada minuto (só funciona na rede de casa).
  static const syncEvery = Duration(minutes: 1);

  Future<void> start() async {
    WidgetsBinding.instance.addObserver(this);
    await reload();
    await loadPeers();
    unawaited(sync(quiet: true));
    _timer = Timer.periodic(syncEvery, (_) => sync(quiet: true));
  }

  @override
  void dispose() {
    _timer?.cancel();
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // Ao voltar para o app (ex.: chegou em casa), sincroniza e relê o board.
    if (state == AppLifecycleState.resumed) {
      unawaited(sync(quiet: true));
    }
  }

  Future<void> reload() async {
    try {
      board = Board.fromJson(jsonDecode(await arca.board(project: project, authors: authors)));
      error = null;
    } catch (e) {
      error = '$e';
    }
    notifyListeners();
  }

  /// Executa uma ação, mostra o erro (se houver) e relê o board.
  Future<bool> run(Future<void> Function() action) async {
    try {
      await action();
      error = null;
      await reload();
      return true;
    } catch (e) {
      error = '$e';
      notifyListeners();
      return false;
    }
  }

  void setProject(String? value) {
    project = value;
    reload();
  }

  void setAuthors(String value) {
    authors = value;
    reload();
  }

  Future<bool> addTask({required String title, String? due, String? project, bool daily = false, String status = 'CART'}) =>
      run(() => arca.addTask(input: jsonEncode({'title': title, 'due': due, 'project': project, 'daily': daily, 'status': status})));

  Future<bool> editTask(int id, {String? title, String? due, String? project, bool? daily}) => run(() => arca.editTask(
      id: id, changes: jsonEncode({'title': title, 'due': due ?? '', 'project': project ?? '', 'daily': daily})));

  Future<bool> moveTask(int id, String status) => run(() => arca.moveTask(id: id, status: status));
  Future<bool> deleteTask(int id) => run(() => arca.deleteTask(id: id));
  Future<bool> approve(int id) => run(() => arca.approveSuggestion(id: id));
  Future<bool> reject(int id) => run(() => arca.rejectSuggestion(id: id));
  Future<bool> addProject(String name, String policy) => run(() => arca.addProject(name: name, policy: policy));
  Future<bool> setPolicy(int id, String policy) => run(() => arca.editProject(id: id, changes: jsonEncode({'policy': policy})));
  Future<bool> deleteProject(int id) => run(() => arca.deleteProject(id: id));

  Future<List<HistoryView>> history() async =>
      (jsonDecode(await arca.history()) as List).map((e) => HistoryView.fromJson(e)).toList();

  Future<List<StatusSnapshot>> statusSnapshots() async =>
      (jsonDecode(await arca.statusSnapshots()) as List).map((e) => StatusSnapshot.fromJson(e)).toList();

  /// Board sem filtro de projeto nem de autor, para os gráficos.
  Future<Board> boardAll() async => Board.fromJson(jsonDecode(await arca.board(project: null, authors: 'all')));

  // ---------- Sincronização ----------

  Future<void> loadPeers() async {
    try {
      peers = (jsonDecode(await arca.peers()) as List).map((e) => Peer.fromJson(e)).toList();
    } catch (e) {
      error = '$e';
    }
    notifyListeners();
  }

  /// Pareia com o PC a partir do conteúdo do QR code e já sincroniza.
  Future<bool> pair(String code) async {
    try {
      await arca.pair(code: code);
      await loadPeers();
      await sync();
      return true;
    } catch (e) {
      error = '$e';
      notifyListeners();
      return false;
    }
  }

  Future<void> unpair(String device) async {
    await run(() => arca.unpair(device: device));
    await loadPeers();
  }

  /// `quiet`: falhas (ex.: fora de casa) não viram mensagem de erro, só ficam em `lastSyncMessage`.
  Future<void> sync({bool quiet = false}) async {
    if (syncing || peers.isEmpty) return;
    syncing = true;
    notifyListeners();
    try {
      final results = jsonDecode(await arca.syncNow()) as List;
      final failed = results.where((r) => r['ok'] != true).toList();
      final now = DateTime.now();
      final time = '${now.hour.toString().padLeft(2, '0')}:${now.minute.toString().padLeft(2, '0')}';
      lastSyncOk = failed.isEmpty;
      if (failed.isEmpty) {
        lastSyncMessage = 'Sincronizado às $time';
      } else {
        lastSyncMessage = 'Sem conexão com o PC ($time)';
        if (!quiet) error = failed.map((r) => '${r['peer']}: ${r['error']}').join('\n');
      }
    } catch (e) {
      lastSyncOk = false;
      if (!quiet) error = '$e';
    } finally {
      syncing = false;
      await loadPeers();
      await reload();
    }
  }

  void clearError() {
    error = null;
    notifyListeners();
  }
}
