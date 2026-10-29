import 'dart:io';

import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';

import 'models.dart';
import 'store.dart';

// ---------- Tarefas ----------

class TaskList extends StatelessWidget {
  final ArcaStore store;
  final List<TaskView> tasks;
  final String empty;
  final bool showProject;

  const TaskList({super.key, required this.store, required this.tasks, required this.empty, required this.showProject});

  @override
  Widget build(BuildContext context) {
    if (tasks.isEmpty) {
      // ListView para o "puxar para atualizar" funcionar mesmo vazio.
      return ListView(children: [
        Padding(
          padding: const EdgeInsets.all(48),
          child: Text(empty, textAlign: TextAlign.center, style: TextStyle(color: Theme.of(context).hintColor)),
        ),
      ]);
    }
    return ListView.builder(
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 96),
      itemCount: tasks.length,
      itemBuilder: (context, i) => TaskCard(store: store, task: tasks[i], showProject: showProject),
    );
  }
}

/// Deslizar para a direita avança (estante → carrinho → feita);
/// para a esquerda, volta (feita → carrinho → estante).
class TaskCard extends StatelessWidget {
  final ArcaStore store;
  final TaskView task;
  final bool showProject;

  const TaskCard({super.key, required this.store, required this.task, required this.showProject});

  String? get _forward => switch (task.status) { 'SHELF' => 'CART', 'CART' => 'DONE', _ => null };
  String? get _back => switch (task.status) { 'DONE' => 'CART', 'CART' => 'SHELF', _ => null };

  static const _labels = {'CART': 'Carrinho', 'SHELF': 'Estante', 'DONE': 'Feita'};

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final done = task.status == 'DONE';

    Widget swipeBackground(String? target, Alignment alignment, Color color) => Container(
          alignment: alignment,
          padding: const EdgeInsets.symmetric(horizontal: 20),
          decoration: BoxDecoration(color: color, borderRadius: BorderRadius.circular(12)),
          child: Text(target == null ? '' : _labels[target]!, style: TextStyle(color: scheme.onPrimary, fontWeight: FontWeight.w600)),
        );

    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Dismissible(
        key: ValueKey('${task.id}-${task.status}'),
        direction: _forward == null
            ? DismissDirection.endToStart
            : _back == null
                ? DismissDirection.startToEnd
                : DismissDirection.horizontal,
        background: swipeBackground(_forward, Alignment.centerLeft, scheme.primary),
        secondaryBackground: swipeBackground(_back, Alignment.centerRight, scheme.tertiary),
        confirmDismiss: (direction) async {
          final target = direction == DismissDirection.startToEnd ? _forward : _back;
          if (target != null) await store.moveTask(task.id, target);
          return false; // o board recarregado já tira o cartão da lista
        },
        child: Card(
          // Sem isso o Dismissible deixa o cartão com a largura do conteúdo.
          clipBehavior: Clip.antiAlias,
          margin: EdgeInsets.zero,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
            side: task.agent != null ? BorderSide(color: Colors.amber.shade700, width: 1.2) : BorderSide.none,
          ),
          child: InkWell(
            borderRadius: BorderRadius.circular(12),
            onTap: () => showTaskSheet(context, store, task: task),
            child: Container(
              width: double.infinity,
              padding: const EdgeInsets.all(12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    task.title,
                    style: TextStyle(
                      fontSize: 15,
                      decoration: done ? TextDecoration.lineThrough : null,
                      color: done ? Theme.of(context).hintColor : null,
                    ),
                  ),
                  Wrap(spacing: 6, runSpacing: 4, children: [
                    if (task.daily) _Tag('diária', Colors.blue),
                    if (task.dueLabel != null)
                      _Tag(task.dueLabel!, switch (task.dueState) { 'overdue' => Colors.red, 'today' => Colors.orange, _ => Colors.indigo }),
                    if (showProject && task.project != null) _Tag('@${task.project}', Colors.purple),
                    if (task.agent != null) _Tag(task.agent!, Colors.amber.shade800),
                    for (final a in task.workingAgents) _Tag('⚙ $a', Colors.green),
                  ]),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _Tag extends StatelessWidget {
  final String text;
  final Color color;
  const _Tag(this.text, this.color);

  @override
  Widget build(BuildContext context) => Container(
        margin: const EdgeInsets.only(top: 6),
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 1),
        decoration: BoxDecoration(border: Border.all(color: color), borderRadius: BorderRadius.circular(99)),
        child: Text(text, style: TextStyle(fontSize: 12, color: color)),
      );
}

/// Formulário de tarefa: cria (sem `task`) ou edita.
Future<void> showTaskSheet(BuildContext context, ArcaStore store, {TaskView? task, String defaultStatus = 'CART'}) {
  final title = TextEditingController(text: task?.title ?? '');
  final due = TextEditingController(text: task?.dueDate ?? '');
  String? project = task?.project ?? store.project;
  bool daily = task?.daily ?? false;
  String status = defaultStatus;
  final projects = store.board?.projects ?? [];

  return showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    showDragHandle: true,
    builder: (context) => StatefulBuilder(
      builder: (context, setState) => Padding(
        padding: EdgeInsets.fromLTRB(20, 0, 20, MediaQuery.of(context).viewInsets.bottom + 20),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(task == null ? 'Nova tarefa' : 'Tarefa #${task.id}', style: Theme.of(context).textTheme.titleLarge),
            const SizedBox(height: 12),
            TextField(controller: title, autofocus: task == null, decoration: const InputDecoration(labelText: 'Título')),
            TextField(controller: due, decoration: const InputDecoration(labelText: 'Prazo', hintText: '+3d, amanhã, 2026-11-05')),
            DropdownButtonFormField<String?>(
              initialValue: projects.any((p) => p.name == project) ? project : null,
              decoration: const InputDecoration(labelText: 'Projeto'),
              items: [
                const DropdownMenuItem(value: null, child: Text('sem projeto')),
                for (final p in projects) DropdownMenuItem(value: p.name, child: Text(p.name)),
              ],
              onChanged: (v) => setState(() => project = v),
            ),
            SwitchListTile(contentPadding: EdgeInsets.zero, title: const Text('Diária'), value: daily, onChanged: (v) => setState(() => daily = v)),
            if (task == null)
              SegmentedButton<String>(
                segments: const [
                  ButtonSegment(value: 'CART', label: Text('Carrinho')),
                  ButtonSegment(value: 'SHELF', label: Text('Estante')),
                ],
                selected: {status},
                onSelectionChanged: (s) => setState(() => status = s.first),
              ),
            if (task?.agent != null) Padding(padding: const EdgeInsets.only(top: 8), child: Text('Criada por ${task!.agent}.')),
            const SizedBox(height: 16),
            Row(children: [
              if (task != null)
                TextButton.icon(
                  onPressed: () async {
                    Navigator.pop(context);
                    await store.deleteTask(task.id);
                  },
                  icon: const Icon(Icons.delete_outline),
                  label: const Text('Apagar'),
                ),
              const Spacer(),
              FilledButton(
                onPressed: () async {
                  if (title.text.trim().isEmpty) return;
                  final ok = task == null
                      ? await store.addTask(title: title.text, due: due.text, project: project, daily: daily, status: status)
                      : await store.editTask(task.id, title: title.text, due: due.text, project: project, daily: daily);
                  if (ok && context.mounted) Navigator.pop(context);
                },
                child: Text(task == null ? 'Adicionar' : 'Salvar'),
              ),
            ]),
          ],
        ),
      ),
    ),
  );
}

// ---------- Agentes e sugestões ----------

class AgentsPanel extends StatelessWidget {
  final ArcaStore store;
  final Board board;
  const AgentsPanel({super.key, required this.store, required this.board});

  Color _dot(String activity) => switch (activity) {
        'working' => Colors.green,
        'waiting' => Colors.blue,
        'possibly_stopped' => Colors.orange,
        _ => Colors.grey,
      };

  @override
  Widget build(BuildContext context) {
    final text = Theme.of(context).textTheme;
    return ListView(padding: const EdgeInsets.all(16), children: [
      Text('Sugestões dos agentes', style: text.titleMedium),
      if (board.suggestions.isEmpty) const Padding(padding: EdgeInsets.symmetric(vertical: 8), child: Text('Nada esperando você.')),
      for (final s in board.suggestions)
        Card(
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text.rich(TextSpan(children: [
                TextSpan(text: s.agent, style: const TextStyle(fontWeight: FontWeight.bold)),
                const TextSpan(text: ' propõe '),
                TextSpan(text: s.summary, style: const TextStyle(fontWeight: FontWeight.bold)),
                const TextSpan(text: ' em '),
                TextSpan(text: s.taskTitle, style: const TextStyle(fontStyle: FontStyle.italic)),
              ])),
              if (s.reason != null) Padding(padding: const EdgeInsets.only(top: 4), child: Text('“${s.reason}”', style: text.bodySmall)),
              Row(children: [
                Text(s.createdLabel, style: text.bodySmall),
                const Spacer(),
                TextButton(onPressed: () => store.reject(s.id), child: const Text('Rejeitar')),
                FilledButton(onPressed: () => store.approve(s.id), child: const Text('Aprovar')),
              ]),
            ]),
          ),
        ),
      const SizedBox(height: 20),
      Text('Sessões', style: text.titleMedium),
      if (board.sessions.isEmpty)
        const Padding(
          padding: EdgeInsets.symmetric(vertical: 8),
          child: Text('Nenhuma sessão recente. As sessões chegam do PC na sincronização.'),
        ),
      for (final s in board.sessions)
        ListTile(
          contentPadding: EdgeInsets.zero,
          leading: Icon(Icons.circle, size: 12, color: _dot(s.activity)),
          title: Text('${s.agent} · ${s.activityLabel}'),
          subtitle: Text([
            s.project ?? 'sem projeto',
            'última atividade ${s.lastSeenLabel}',
            if (s.taskTitle != null) 'em ${s.taskTitle}',
          ].join(' · ')),
        ),
    ]);
  }
}

// ---------- Histórico e projetos ----------

void openHistory(BuildContext context, ArcaStore store) {
  Navigator.push(context, MaterialPageRoute(builder: (_) => Scaffold(
        appBar: AppBar(title: const Text('Histórico')),
        body: FutureBuilder<List<HistoryView>>(
          future: store.history(),
          builder: (context, snap) {
            if (!snap.hasData) return const Center(child: CircularProgressIndicator());
            final items = snap.data!;
            if (items.isEmpty) return const Center(child: Text('Nada concluído ainda.'));
            return ListView.separated(
              itemCount: items.length,
              separatorBuilder: (_, _) => const Divider(height: 1),
              itemBuilder: (_, i) => ListTile(
                title: Text(items[i].title),
                subtitle: Text([items[i].date, if (items[i].project != null) '@${items[i].project}', if (items[i].agent != null) items[i].agent!].join(' · ')),
                trailing: items[i].daily ? const Text('diária') : null,
              ),
            );
          },
        ),
      )));
}

const _policies = {
  'NONE': 'Agentes só leem',
  'OWN': 'Agentes só mexem no que criaram',
  'ALL': 'Agentes gerenciam tudo',
};

void openProjects(BuildContext context, ArcaStore store) {
  Navigator.push(context, MaterialPageRoute(builder: (_) => ListenableBuilder(
        listenable: store,
        builder: (context, _) {
          final projects = store.board?.projects ?? [];
          return Scaffold(
            appBar: AppBar(title: const Text('Projetos')),
            floatingActionButton: FloatingActionButton(
              onPressed: () async {
                final name = TextEditingController();
                final ok = await showDialog<bool>(
                  context: context,
                  builder: (context) => AlertDialog(
                    title: const Text('Novo projeto'),
                    content: TextField(controller: name, autofocus: true, decoration: const InputDecoration(labelText: 'Nome')),
                    actions: [
                      TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancelar')),
                      FilledButton(onPressed: () => Navigator.pop(context, true), child: const Text('Criar')),
                    ],
                  ),
                );
                if (ok == true && name.text.trim().isNotEmpty) await store.addProject(name.text, 'OWN');
              },
              child: const Icon(Icons.add),
            ),
            body: projects.isEmpty
                ? const Center(child: Text('Nenhum projeto. As pastas vinculadas ficam no PC.'))
                : ListView(children: [
                    for (final p in projects)
                      ListTile(
                        title: Text(p.name),
                        subtitle: Text('estante ${p.shelf} · carrinho ${p.cart} · feitas ${p.done}\n${_policies[p.policy]}'),
                        isThreeLine: true,
                        trailing: PopupMenuButton<String>(
                          onSelected: (v) => v == 'delete' ? store.deleteProject(p.id) : store.setPolicy(p.id, v),
                          itemBuilder: (_) => [
                            for (final e in _policies.entries) CheckedPopupMenuItem(value: e.key, checked: p.policy == e.key, child: Text(e.value)),
                            const PopupMenuDivider(),
                            const PopupMenuItem(value: 'delete', child: Text('Apagar (as tarefas ficam)')),
                          ],
                        ),
                      ),
                  ]),
          );
        },
      )));
}

// ---------- Sincronização ----------

void openSync(BuildContext context, ArcaStore store) {
  Navigator.push(context, MaterialPageRoute(builder: (_) => SyncScreen(store: store)));
}

class SyncScreen extends StatelessWidget {
  final ArcaStore store;
  const SyncScreen({super.key, required this.store});

  Future<void> _pairWithCode(BuildContext context) async {
    final code = TextEditingController();
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Código de pareamento'),
        content: TextField(controller: code, maxLines: 4, decoration: const InputDecoration(hintText: 'Cole o código mostrado no PC')),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancelar')),
          FilledButton(onPressed: () => Navigator.pop(context, true), child: const Text('Parear')),
        ],
      ),
    );
    if (ok == true) await store.pair(code.text);
  }

  Future<void> _scan(BuildContext context) async {
    final code = await Navigator.push<String>(context, MaterialPageRoute(builder: (_) => const _ScannerScreen()));
    if (code != null) await store.pair(code);
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: store,
      builder: (context, _) => Scaffold(
        appBar: AppBar(title: const Text('Sincronização')),
        body: ListView(padding: const EdgeInsets.all(16), children: [
          const Text(
            'O PC é o servidor: abra o app desktop (ou rode `arca serve`) e leia o QR code. '
            'Depois disso, o celular sincroniza sozinho sempre que estiver na mesma rede.',
          ),
          if (store.error != null)
            Padding(padding: const EdgeInsets.only(top: 12), child: Text(store.error!, style: TextStyle(color: Theme.of(context).colorScheme.error))),
          const SizedBox(height: 16),
          for (final peer in store.peers)
            Card(
              child: ListTile(
                leading: const Icon(Icons.computer),
                title: Text(peer.name),
                subtitle: Text('${peer.primaryUrl}\núltima sincronização: ${peer.lastSyncedAt ?? 'nunca'}'),
                isThreeLine: true,
                trailing: IconButton(tooltip: 'Desparear', icon: const Icon(Icons.link_off), onPressed: () => store.unpair(peer.device)),
              ),
            ),
          if (store.lastSyncMessage != null) Padding(padding: const EdgeInsets.symmetric(vertical: 8), child: Text(store.lastSyncMessage!)),
          const SizedBox(height: 8),
          if (store.peers.isNotEmpty)
            FilledButton.icon(
              onPressed: store.syncing ? null : () => store.sync(),
              icon: const Icon(Icons.sync),
              label: Text(store.syncing ? 'Sincronizando…' : 'Sincronizar agora'),
            ),
          const SizedBox(height: 8),
          if (Platform.isAndroid)
            OutlinedButton.icon(onPressed: () => _scan(context), icon: const Icon(Icons.qr_code_scanner), label: const Text('Ler QR code do PC')),
          TextButton(onPressed: () => _pairWithCode(context), child: const Text('Colar código de pareamento')),
        ]),
      ),
    );
  }
}

class _ScannerScreen extends StatefulWidget {
  const _ScannerScreen();

  @override
  State<_ScannerScreen> createState() => _ScannerScreenState();
}

class _ScannerScreenState extends State<_ScannerScreen> {
  bool _done = false;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Aponte para o QR code')),
      body: MobileScanner(
        onDetect: (capture) {
          final value = capture.barcodes.map((b) => b.rawValue).whereType<String>().where((v) => v.contains('"arca"')).firstOrNull;
          if (value != null && !_done) {
            _done = true;
            Navigator.pop(context, value);
          }
        },
      ),
    );
  }
}
