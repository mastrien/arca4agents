import 'dart:io';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:package_info_plus/package_info_plus.dart';

import 'models.dart';
import 'stats.dart';
import 'store.dart';
import 'theme.dart';

// ---------- Peças comuns ----------

class ArcaMark extends StatelessWidget {
  final double size;
  const ArcaMark({super.key, this.size = 26});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return CustomPaint(
      size: Size.square(size),
      painter: _ArcaMarkPainter(ground: c.ground, accent: c.accent),
    );
  }
}

class _ArcaMarkPainter extends CustomPainter {
  final Color ground;
  final Color accent;

  const _ArcaMarkPainter({required this.ground, required this.accent});

  @override
  void paint(Canvas canvas, Size size) {
    canvas.save();
    canvas.scale(size.width / 12.7, size.height / 12.7);

    const center = Offset(6.3499999, 6.3499999);
    const circleRadius = 4.9857421;
    canvas.drawCircle(center, circleRadius, Paint()..color = ground);
    canvas.drawCircle(
      center,
      circleRadius,
      Paint()
        ..color = accent
        ..style = PaintingStyle.stroke
        ..strokeWidth = 0.352,
    );

    const r = Radius.circular(1.7202544);
    final rightLeaf = Path()
      ..moveTo(8.3761448, 5.9760613)
      ..arcToPoint(const Offset(7.1707552, 6.4703316), radius: r, largeArc: false, clockwise: false)
      ..arcToPoint(const Offset(8.3761448, 6.9646019), radius: r, largeArc: false, clockwise: false)
      ..arcToPoint(const Offset(9.4828015, 6.5599786), radius: r, largeArc: false, clockwise: false)
      ..lineTo(11.274532, 6.513338)
      ..lineTo(9.5809287, 6.4697259)
      ..arcToPoint(const Offset(8.3761448, 5.9760613), radius: r, largeArc: false, clockwise: false)
      ..close();

    final leftLeaf = Path()
      ..moveTo(4.3601986, 5.9760613)
      ..arcToPoint(const Offset(3.1566261, 6.4685144), radius: r, largeArc: false, clockwise: false)
      ..lineTo(1.4254685, 6.513338)
      ..lineTo(3.2535419, 6.5605844)
      ..arcToPoint(const Offset(3.2541475, 6.5611902), radius: r, largeArc: false, clockwise: false)
      ..arcToPoint(const Offset(4.3601986, 6.9646019), radius: r, largeArc: false, clockwise: false)
      ..arcToPoint(const Offset(5.5655882, 6.4703316), radius: r, largeArc: false, clockwise: false)
      ..arcToPoint(const Offset(4.3601986, 5.9760613), radius: r, largeArc: false, clockwise: false)
      ..close();

    final topLeaf = Path()
      ..moveTo(6.3681717, 3.2569689)
      ..arcToPoint(const Offset(5.8739013, 4.4623585), radius: r, largeArc: false, clockwise: false)
      ..arcToPoint(const Offset(6.2894276, 5.582341), radius: r, largeArc: false, clockwise: false)
      ..lineTo(6.3681712, 8.6170156)
      ..lineTo(6.4469148, 5.582341)
      ..arcToPoint(const Offset(6.862442, 4.4623585), radius: r, largeArc: false, clockwise: false)
      ..arcToPoint(const Offset(6.3681717, 3.2569689), radius: r, largeArc: false, clockwise: false)
      ..close();

    final leafPaint = Paint()..color = accent;
    canvas.drawPath(rightLeaf, leafPaint);
    canvas.drawPath(leftLeaf, leafPaint);
    canvas.drawPath(topLeaf, leafPaint);
    canvas.restore();
  }

  @override
  bool shouldRepaint(_ArcaMarkPainter oldDelegate) => oldDelegate.ground != ground || oldDelegate.accent != accent;
}

class ProjectDot extends StatelessWidget {
  final String name;
  final double size;
  const ProjectDot(this.name, {super.key, this.size = 8});

  @override
  Widget build(BuildContext context) => Container(
        width: size,
        height: size,
        decoration: BoxDecoration(color: projectColor(name), borderRadius: BorderRadius.circular(2)),
      );
}

class StatusDot extends StatelessWidget {
  final String activity;
  const StatusDot(this.activity, {super.key});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final hollow = activity == 'possibly_stopped' || activity == 'ended';
    return Container(
      width: 9,
      height: 9,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: hollow ? null : (activity == 'waiting' ? c.attentionDot : c.ok),
        border: hollow ? Border.all(color: c.idle, width: 1.5) : null,
      ),
    );
  }
}

/// Bloco branco com borda, a "superfície" das listas.
class Panel extends StatelessWidget {
  final Widget child;
  final EdgeInsetsGeometry padding;
  final Color? color;
  final Color? border;
  const Panel({super.key, required this.child, this.padding = const EdgeInsets.fromLTRB(16, 14, 16, 14), this.color, this.border});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Container(
      width: double.infinity,
      padding: padding,
      decoration: BoxDecoration(
        color: color ?? c.surface,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: border ?? c.line),
      ),
      child: child,
    );
  }
}

class SectionTitle extends StatelessWidget {
  final String text;
  final Color? color;
  const SectionTitle(this.text, {super.key, this.color});

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(bottom: 8),
        child: Text(text, style: TextStyle(fontSize: 13, fontWeight: FontWeight.w600, color: color ?? context.arca.muted)),
      );
}

class ScreenHeader extends StatelessWidget {
  final String title;
  final Widget? trailing;
  final bool back;
  const ScreenHeader(this.title, {super.key, this.trailing, this.back = false});

  @override
  Widget build(BuildContext context) => Padding(
        padding: EdgeInsets.fromLTRB(back ? 4 : 16, 12, 12, 8),
        child: Row(children: [
          if (back) IconButton(tooltip: 'Voltar', onPressed: () => Navigator.pop(context), icon: const Icon(Icons.arrow_back)),
          Expanded(child: Text(title, style: TextStyle(fontSize: back ? 19 : 22, fontWeight: back ? FontWeight.w600 : FontWeight.w700))),
          ?trailing,
        ]),
      );
}

class AgentChip extends StatelessWidget {
  final String agent;
  const AgentChip(this.agent, {super.key});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
      decoration: BoxDecoration(color: c.agentTint, borderRadius: BorderRadius.circular(5)),
      child: Text(agent, style: TextStyle(fontFamily: mono, fontSize: 11, color: c.agent)),
    );
  }
}

// ---------- Tarefas ----------

class TasksScreen extends StatelessWidget {
  final ArcaStore store;
  final Board board;
  final String list;
  final ValueChanged<String> onList;
  final VoidCallback onAttention;
  final VoidCallback onOpenDates;

  const TasksScreen({
    super.key,
    required this.store,
    required this.board,
    required this.list,
    required this.onList,
    required this.onAttention,
    required this.onOpenDates,
  });

  static const _lists = [('SHELF', 'Estante'), ('CART', 'Carrinho'), ('DONE', 'Feito')];

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final waiting = board.sessions.where((s) => s.activity == 'waiting').toList();
    final tasks = board.tasks(list);
    final showProject = store.project == null;

    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Padding(
        padding: const EdgeInsets.fromLTRB(16, 10, 6, 10),
        child: Row(children: [
          const ArcaMark(),
          const SizedBox(width: 8),
          Text.rich(
            TextSpan(children: [
              const TextSpan(text: 'Arca'),
              TextSpan(text: '4agents', style: TextStyle(color: c.accent)),
            ]),
            style: const TextStyle(fontSize: 19, fontWeight: FontWeight.w700),
          ),
          const Spacer(),
          _ProjectFilter(store: store, board: board),
          _SyncButton(store: store),
        ]),
      ),
      if (waiting.isNotEmpty)
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
          child: Material(
            color: c.attentionTint,
            borderRadius: BorderRadius.circular(12),
            child: InkWell(
              borderRadius: BorderRadius.circular(12),
              onTap: onAttention,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(12, 12, 8, 12),
                child: Row(children: [
                  Container(width: 9, height: 9, decoration: BoxDecoration(shape: BoxShape.circle, color: c.attentionDot)),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text.rich(
                      TextSpan(children: [
                        TextSpan(text: waiting.first.agent, style: const TextStyle(fontWeight: FontWeight.w600)),
                        TextSpan(text: waiting.length == 1 ? ' está esperando você' : ' e mais ${waiting.length - 1} esperando você'),
                        if (waiting.length == 1 && waiting.first.project != null) TextSpan(text: ' em ${waiting.first.project}'),
                      ]),
                      style: TextStyle(fontSize: 13, color: c.attention),
                    ),
                  ),
                  Icon(Icons.chevron_right, size: 18, color: c.attention),
                ]),
              ),
            ),
          ),
        ),
      if (board.upcomingSoon.isNotEmpty)
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
          child: _UpcomingStrip(items: board.upcomingSoon, onTap: onOpenDates),
        ),
      Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16),
        child: Container(
          padding: const EdgeInsets.all(4),
          decoration: BoxDecoration(color: c.recessed, borderRadius: BorderRadius.circular(12)),
          child: Row(children: [
            for (final (status, label) in _lists)
              Expanded(
                child: Semantics(
                  selected: list == status,
                  button: true,
                  child: GestureDetector(
                    onTap: () => onList(status),
                    child: AnimatedContainer(
                      duration: const Duration(milliseconds: 150),
                      height: 40,
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        color: list == status ? c.surface : Colors.transparent,
                        borderRadius: BorderRadius.circular(9),
                        boxShadow: list == status ? [BoxShadow(color: Colors.black.withValues(alpha: 0.06), blurRadius: 2, offset: const Offset(0, 1))] : null,
                      ),
                      child: Text.rich(TextSpan(children: [
                        TextSpan(
                          text: '$label ',
                          style: TextStyle(fontWeight: list == status ? FontWeight.w600 : FontWeight.w400, color: list == status ? c.accent : c.text2),
                        ),
                        TextSpan(text: '${board.tasks(status).length}', style: TextStyle(fontFamily: mono, fontSize: 12, color: c.muted)),
                      ])),
                    ),
                  ),
                ),
              ),
          ]),
        ),
      ),
      Expanded(
        child: RefreshIndicator(
          onRefresh: () => store.peers.isEmpty ? store.reload() : store.sync(),
          child: tasks.isEmpty
              // ListView para o "puxar para atualizar" funcionar mesmo vazio.
              ? ListView(children: [
                  Padding(
                    padding: const EdgeInsets.all(48),
                    child: Text(
                      switch (list) { 'CART' => 'Carrinho vazio.\nPegue algo da estante.', 'SHELF' => 'Estante vazia.', _ => 'Nada concluído hoje.' },
                      textAlign: TextAlign.center,
                      style: TextStyle(color: c.muted),
                    ),
                  ),
                ])
              : ListView(
                  padding: const EdgeInsets.fromLTRB(16, 14, 16, 96),
                  children: [
                    for (final task in tasks)
                      list == 'DONE' ? DoneRow(store: store, task: task, showProject: showProject) : TaskCard(store: store, task: task, showProject: showProject),
                    Padding(
                      padding: const EdgeInsets.only(top: 4),
                      child: Text(
                        switch (list) {
                          'SHELF' => 'Deslize para a direita para levar ao carrinho',
                          'CART' => 'Deslize para a direita para concluir, para a esquerda para mandar à estante',
                          _ => 'Deslize para a esquerda para reabrir',
                        },
                        textAlign: TextAlign.center,
                        style: TextStyle(fontSize: 12, color: c.muted),
                      ),
                    ),
                  ],
                ),
        ),
      ),
    ]);
  }
}

class _ProjectFilter extends StatelessWidget {
  final ArcaStore store;
  final Board board;
  const _ProjectFilter({required this.store, required this.board});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return PopupMenuButton<String>(
      tooltip: 'Filtrar por projeto',
      onSelected: (value) => store.setProject(value.isEmpty ? null : value),
      itemBuilder: (_) => [
        CheckedPopupMenuItem(value: '', checked: store.project == null, child: const Text('Todos os projetos')),
        for (final p in board.projects)
          CheckedPopupMenuItem(
            value: p.name,
            checked: store.project == p.name,
            child: Row(children: [ProjectDot(p.name), const SizedBox(width: 8), Text(p.name)]),
          ),
      ],
      child: Container(
        height: 36,
        padding: const EdgeInsets.symmetric(horizontal: 12),
        decoration: BoxDecoration(color: c.surface, border: Border.all(color: c.line), borderRadius: BorderRadius.circular(99)),
        child: Row(mainAxisSize: MainAxisSize.min, children: [
          if (store.project != null) ...[ProjectDot(store.project!), const SizedBox(width: 6)],
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 120),
            child: Text(store.project ?? 'Todos os projetos', overflow: TextOverflow.ellipsis, style: const TextStyle(fontSize: 13)),
          ),
          const SizedBox(width: 4),
          Icon(Icons.expand_more, size: 16, color: c.muted),
        ]),
      ),
    );
  }
}

class _SyncButton extends StatelessWidget {
  final ArcaStore store;
  const _SyncButton({required this.store});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final unpaired = store.peers.isEmpty;
    final dot = unpaired || store.lastSyncOk == null ? null : (store.lastSyncOk! ? c.ok : c.attentionDot);
    return IconButton(
      tooltip: unpaired ? 'Parear com o PC' : (store.lastSyncMessage ?? 'Sincronizar'),
      onPressed: unpaired
          ? () => openPair(context, store)
          : store.syncing
              ? null
              : () => store.sync(),
      icon: store.syncing
          ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
          : Stack(clipBehavior: Clip.none, children: [
              Icon(unpaired ? Icons.sync_disabled : Icons.sync, color: c.text2),
              if (dot != null)
                Positioned(
                  right: -2,
                  top: -2,
                  child: Container(
                    width: 10,
                    height: 10,
                    decoration: BoxDecoration(shape: BoxShape.circle, color: dot, border: Border.all(color: c.ground, width: 2)),
                  ),
                ),
            ]),
    );
  }
}

class _TaskMeta extends StatelessWidget {
  final TaskView task;
  final bool showProject;
  const _TaskMeta({required this.task, required this.showProject});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final style = TextStyle(fontSize: 13, color: c.muted);
    final items = <Widget>[
      if (showProject && task.project != null)
        Row(mainAxisSize: MainAxisSize.min, children: [ProjectDot(task.project!), const SizedBox(width: 5), Text(task.project!, style: style)]),
      if (task.daily) Row(mainAxisSize: MainAxisSize.min, children: [Icon(Icons.replay, size: 13, color: c.muted), const SizedBox(width: 4), Text('diária', style: style)]),
      if (task.dueLabel != null)
        Text(
          task.dueLabel!,
          style: switch (task.dueState) {
            'overdue' => style.copyWith(color: c.attention, fontWeight: FontWeight.w600),
            'today' => style.copyWith(color: c.accent, fontWeight: FontWeight.w600),
            _ => style,
          },
        ),
      if (task.agent != null) AgentChip(task.agent!),
    ];
    if (items.isEmpty) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Wrap(spacing: 10, runSpacing: 4, crossAxisAlignment: WrapCrossAlignment.center, children: items),
    );
  }
}

/// Deslizar para a direita avança (estante → carrinho → feita); para a esquerda, volta.
class TaskCard extends StatelessWidget {
  final ArcaStore store;
  final TaskView task;
  final bool showProject;

  const TaskCard({super.key, required this.store, required this.task, required this.showProject});

  String? get _forward => switch (task.status) { 'SHELF' => 'CART', 'CART' => 'DONE', _ => null };
  String? get _back => switch (task.status) { 'DONE' => 'CART', 'CART' => 'SHELF', _ => null };

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final working = task.workingAgents.isNotEmpty;
    final shelf = task.status == 'SHELF';

    Widget swipe(String? target, bool forward) => Container(
          alignment: forward ? Alignment.centerLeft : Alignment.centerRight,
          padding: const EdgeInsets.symmetric(horizontal: 18),
          decoration: BoxDecoration(color: forward ? c.accent : c.muted, borderRadius: BorderRadius.circular(14)),
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            Icon(
              switch (target) { 'DONE' => Icons.check, 'CART' => forward ? Icons.arrow_forward : Icons.undo, _ => Icons.arrow_back },
              color: Colors.white,
              size: 18,
            ),
            const SizedBox(width: 8),
            Text(
              switch (target) { 'DONE' => 'Concluir', 'CART' => forward ? 'Carrinho' : 'Reabrir', 'SHELF' => 'Estante', _ => '' },
              style: const TextStyle(color: Colors.white, fontWeight: FontWeight.w600),
            ),
          ]),
        );

    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Dismissible(
        key: ValueKey('${task.id}-${task.status}'),
        direction: _forward == null
            ? DismissDirection.endToStart
            : _back == null
                ? DismissDirection.startToEnd
                : DismissDirection.horizontal,
        background: swipe(_forward, true),
        secondaryBackground: swipe(_back, false),
        confirmDismiss: (direction) async {
          final target = direction == DismissDirection.startToEnd ? _forward : _back;
          if (target != null) await moveWithUndo(context, store, task, target);
          return false; // o board recarregado já tira o cartão da lista
        },
        child: Material(
          color: shelf ? c.surface.withValues(alpha: 0.7) : c.surface,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(14),
            side: BorderSide(color: working ? c.accentLine : c.line),
          ),
          child: InkWell(
            borderRadius: BorderRadius.circular(14),
            onTap: () => showTaskSheet(context, store, task: task),
            child: Container(
              // Sem a largura cheia o Dismissible deixa o cartão com a largura do conteúdo.
              width: double.infinity,
              padding: const EdgeInsets.fromLTRB(16, 14, 16, 14),
              child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                Text(task.title, style: TextStyle(fontSize: 16, fontWeight: shelf ? FontWeight.w400 : FontWeight.w500, color: shelf ? c.text2 : c.text)),
                _TaskMeta(task: task, showProject: showProject),
                if (working)
                  Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: Row(children: [
                      Container(width: 7, height: 7, decoration: BoxDecoration(shape: BoxShape.circle, color: c.ok)),
                      const SizedBox(width: 8),
                      Expanded(child: Text('${task.workingAgents.join(', ')} trabalhando nisto', style: TextStyle(fontSize: 12, color: c.accent))),
                    ]),
                  ),
              ]),
            ),
          ),
        ),
      ),
    );
  }
}

class DoneRow extends StatelessWidget {
  final ArcaStore store;
  final TaskView task;
  final bool showProject;
  const DoneRow({super.key, required this.store, required this.task, required this.showProject});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final details = [
      if (showProject && task.project != null) task.project!,
      if (task.daily) 'diária · volta amanhã',
      if (task.agent != null) 'por ${task.agent}',
      ?task.doneTime,
    ];
    return Dismissible(
      key: ValueKey('${task.id}-${task.status}'),
      direction: DismissDirection.endToStart,
      background: Container(
        alignment: Alignment.centerRight,
        padding: const EdgeInsets.symmetric(horizontal: 18),
        color: c.muted,
        child: const Text('Reabrir', style: TextStyle(color: Colors.white, fontWeight: FontWeight.w600)),
      ),
      confirmDismiss: (_) async {
        await moveWithUndo(context, store, task, 'CART');
        return false;
      },
      child: InkWell(
        onTap: () => showTaskSheet(context, store, task: task),
        child: Container(
          width: double.infinity,
          padding: const EdgeInsets.symmetric(vertical: 12, horizontal: 4),
          decoration: BoxDecoration(border: Border(bottom: BorderSide(color: c.line))),
          child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Padding(padding: const EdgeInsets.only(top: 2), child: Icon(Icons.check, size: 18, color: c.ok)),
            const SizedBox(width: 12),
            Expanded(
              child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                Text(task.title, style: TextStyle(fontSize: 15, color: c.text2)),
                if (details.isNotEmpty) Padding(padding: const EdgeInsets.only(top: 3), child: Text(details.join(' · '), style: TextStyle(fontSize: 12, color: c.muted))),
              ]),
            ),
          ]),
        ),
      ),
    );
  }
}

/// Move a tarefa e mostra um aviso com o que mudou e um "Desfazer", que a devolve para onde estava
/// (para o caso de um deslize sem querer).
Future<void> moveWithUndo(BuildContext context, ArcaStore store, TaskView task, String target) async {
  final messenger = ScaffoldMessenger.of(context);
  final from = task.status;
  if (!await store.moveTask(task.id, target)) return;
  final what = switch ((from, target)) {
    ('DONE', 'CART') => 'reaberta',
    (_, 'DONE') => 'concluída',
    (_, 'CART') => 'foi para o carrinho',
    (_, 'SHELF') => 'foi para a estante',
    _ => 'movida',
  };
  messenger
    ..hideCurrentSnackBar()
    ..showSnackBar(SnackBar(
      content: Text('“${task.title}” $what', maxLines: 2, overflow: TextOverflow.ellipsis),
      duration: const Duration(seconds: 4),
      // Com ação, o Flutter mantém o aviso até ele ser fechado; aqui ele some sozinho.
      persist: false,
      action: SnackBarAction(label: 'Desfazer', onPressed: () => store.moveTask(task.id, from)),
    ));
}

// ---------- Formulário de tarefa ----------

const _weekdays = ['segunda', 'terça', 'quarta', 'quinta', 'sexta', 'sábado', 'domingo'];
const _months = ['janeiro', 'fevereiro', 'março', 'abril', 'maio', 'junho', 'julho', 'agosto', 'setembro', 'outubro', 'novembro', 'dezembro'];

String _iso(DateTime d) => '${d.year}-${d.month.toString().padLeft(2, '0')}-${d.day.toString().padLeft(2, '0')}';

DateTime? _dueDate(String due) {
  final today = DateUtils.dateOnly(DateTime.now());
  return switch (due) {
    '' => null,
    'hoje' => today,
    'amanhã' => today.add(const Duration(days: 1)),
    '+1w' => today.add(const Duration(days: 7)),
    _ => DateTime.tryParse(due),
  };
}

String _dueText(DateTime d) => '${_weekdays[d.weekday - 1]}, ${d.day} de ${_months[d.month - 1]}';

class _Choice extends StatelessWidget {
  final String label;
  final bool selected;
  final VoidCallback onTap;
  final Widget? leading;
  const _Choice({required this.label, required this.selected, required this.onTap, this.leading});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Semantics(
      selected: selected,
      button: true,
      child: Material(
        color: selected ? c.accentTint : c.surface,
        shape: StadiumBorder(side: BorderSide(color: selected ? c.accent : c.line)),
        child: InkWell(
          customBorder: const StadiumBorder(),
          onTap: onTap,
          child: Container(
            height: 40,
            padding: const EdgeInsets.symmetric(horizontal: 14),
            child: Row(mainAxisSize: MainAxisSize.min, children: [
              if (leading != null) ...[leading!, const SizedBox(width: 6)],
              Text(label, style: TextStyle(fontSize: 14, color: selected ? c.accent : c.text2, fontWeight: selected ? FontWeight.w600 : FontWeight.w400)),
            ]),
          ),
        ),
      ),
    );
  }
}

/// Formulário de tarefa: cria (sem `task`) ou edita.
Future<void> showTaskSheet(BuildContext context, ArcaStore store, {TaskView? task}) {
  final title = TextEditingController(text: task?.title ?? '');
  String due = task?.dueDate ?? '';
  String? project = task?.project ?? store.project;
  bool daily = task?.daily ?? false;
  final projects = store.board?.projects ?? [];

  Future<void> save(BuildContext context, String? status) async {
    if (title.text.trim().isEmpty) return;
    final ok = task == null
        ? await store.addTask(title: title.text, due: due, project: project, daily: daily, status: status ?? 'CART')
        : await store.editTask(task.id, title: title.text, due: due, project: project, daily: daily);
    if (ok && context.mounted) Navigator.pop(context);
  }

  return showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    showDragHandle: true,
    builder: (context) => StatefulBuilder(builder: (context, setState) {
      final c = context.arca;
      final date = _dueDate(due);
      final custom = due.isNotEmpty && !['hoje', 'amanhã', '+1w'].contains(due);
      final label = TextStyle(fontSize: 13, fontWeight: FontWeight.w600, color: c.text2);
      return Padding(
        padding: EdgeInsets.fromLTRB(20, 0, 20, MediaQuery.of(context).viewInsets.bottom + 24),
        child: SingleChildScrollView(
          child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            Row(children: [
              Expanded(child: Text(task == null ? 'Nova tarefa' : 'Tarefa #${task.id}', style: const TextStyle(fontSize: 18, fontWeight: FontWeight.w600))),
              TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancelar')),
            ]),
            if (task?.agent != null) Text('criada por ${task!.agent}', style: TextStyle(fontSize: 13, color: c.muted)),
            const SizedBox(height: 14),
            Text('Título', style: label),
            const SizedBox(height: 6),
            TextField(controller: title, autofocus: task == null, textCapitalization: TextCapitalization.sentences, style: const TextStyle(fontSize: 16)),
            const SizedBox(height: 18),
            Text.rich(TextSpan(children: [
              const TextSpan(text: 'Prazo'),
              TextSpan(text: ' · só referência', style: TextStyle(fontWeight: FontWeight.w400, color: c.muted)),
            ]), style: label),
            const SizedBox(height: 8),
            Wrap(spacing: 8, runSpacing: 8, children: [
              for (final (value, text) in [('', 'Sem prazo'), ('hoje', 'Hoje'), ('amanhã', 'Amanhã'), ('+1w', 'Em 1 semana')])
                _Choice(label: text, selected: due == value, onTap: () => setState(() => due = value)),
              _Choice(
                label: custom && date != null ? '${date.day}/${date.month}' : 'Data',
                selected: custom,
                leading: Icon(Icons.calendar_today_outlined, size: 15, color: custom ? c.accent : c.text2),
                onTap: () async {
                  final today = DateUtils.dateOnly(DateTime.now());
                  final picked = await showDatePicker(
                    context: context,
                    initialDate: date ?? today,
                    firstDate: DateTime(today.year - 1),
                    lastDate: DateTime(today.year + 5),
                  );
                  if (picked != null) setState(() => due = _iso(picked));
                },
              ),
            ]),
            if (date != null) Padding(padding: const EdgeInsets.only(top: 8), child: Text(_dueText(date), style: TextStyle(fontSize: 13, color: c.muted))),
            const SizedBox(height: 18),
            Text('Projeto', style: label),
            const SizedBox(height: 8),
            Wrap(spacing: 8, runSpacing: 8, children: [
              _Choice(label: 'Nenhum', selected: project == null, onTap: () => setState(() => project = null)),
              for (final p in projects) _Choice(label: p.name, leading: ProjectDot(p.name), selected: project == p.name, onTap: () => setState(() => project = p.name)),
            ]),
            const SizedBox(height: 10),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Diária', style: TextStyle(fontSize: 15, fontWeight: FontWeight.w500)),
              subtitle: Text('volta ao carrinho todo dia depois de feita', style: TextStyle(fontSize: 13, color: c.muted)),
              value: daily,
              onChanged: (v) => setState(() => daily = v),
            ),
            const SizedBox(height: 12),
            if (task == null)
              Row(children: [
                Expanded(child: OutlinedButton(onPressed: () => save(context, 'SHELF'), child: const Text('Na estante'))),
                const SizedBox(width: 8),
                Expanded(child: FilledButton(onPressed: () => save(context, 'CART'), child: const Text('No carrinho'))),
              ])
            else
              Row(children: [
                Expanded(
                  child: OutlinedButton.icon(
                    onPressed: () async {
                      Navigator.pop(context);
                      await store.deleteTask(task.id);
                    },
                    icon: const Icon(Icons.delete_outline, size: 18),
                    label: const Text('Apagar'),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(child: FilledButton(onPressed: () => save(context, null), child: const Text('Salvar'))),
              ]),
          ]),
        ),
      );
    }),
  );
}

// ---------- Agentes e sugestões ----------

class AgentsScreen extends StatelessWidget {
  final ArcaStore store;
  final Board board;
  const AgentsScreen({super.key, required this.store, required this.board});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final waiting = board.sessions.where((s) => s.activity == 'waiting').toList();
    final pending = waiting.length + board.suggestions.length;

    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      ScreenHeader('Agentes', trailing: store.lastSyncMessage == null ? null : Text(store.lastSyncMessage!, style: TextStyle(fontSize: 12, color: c.muted))),
      Expanded(
        child: RefreshIndicator(
          onRefresh: () => store.peers.isEmpty ? store.reload() : store.sync(),
          child: ListView(padding: const EdgeInsets.fromLTRB(16, 4, 16, 24), children: [
            if (pending > 0) ...[
              SectionTitle('Precisa de você · $pending', color: c.attention),
              for (final s in waiting)
                Padding(
                  padding: const EdgeInsets.only(bottom: 10),
                  child: Panel(
                    color: c.attentionTint,
                    border: c.attentionTint,
                    child: Row(children: [
                      const StatusDot('waiting'),
                      const SizedBox(width: 12),
                      Expanded(
                        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                          Text.rich(TextSpan(children: [
                            TextSpan(text: s.agent, style: const TextStyle(fontWeight: FontWeight.w600)),
                            const TextSpan(text: ' está esperando você'),
                          ]), style: TextStyle(fontSize: 15, color: c.attention)),
                          const SizedBox(height: 2),
                          Text('${s.place} · ${s.lastSeenLabel}${s.remoteUrl == null ? ' · responda no PC' : ''}', style: TextStyle(fontSize: 13, color: c.attention)),
                          if (s.remoteUrl != null) ...[
                            const SizedBox(height: 8),
                            FilledButton.tonalIcon(
                              onPressed: () => openRemote(context, s.remoteUrl!),
                              icon: const Icon(Icons.open_in_new, size: 16),
                              label: const Text('Abrir no Claude'),
                            ),
                          ],
                        ]),
                      ),
                    ]),
                  ),
                ),
              for (final s in board.suggestions)
                Padding(
                  padding: const EdgeInsets.only(bottom: 10),
                  child: Panel(
                    child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                      Text.rich(TextSpan(children: [
                        TextSpan(text: s.agent, style: const TextStyle(fontWeight: FontWeight.w600)),
                        const TextSpan(text: ' propõe '),
                        TextSpan(text: s.summary, style: const TextStyle(fontWeight: FontWeight.w600)),
                        TextSpan(text: ' #${s.taskId} ${s.taskTitle}'),
                      ]), style: const TextStyle(fontSize: 15, height: 1.4)),
                      if (s.reason != null)
                        Padding(
                          padding: const EdgeInsets.only(top: 6),
                          child: Text('“${s.reason}”', style: TextStyle(fontSize: 14, fontStyle: FontStyle.italic, color: c.text2, height: 1.4)),
                        ),
                      Padding(padding: const EdgeInsets.only(top: 4), child: Text(s.createdLabel, style: TextStyle(fontSize: 12, color: c.muted))),
                      const SizedBox(height: 10),
                      Row(children: [
                        Expanded(child: OutlinedButton(onPressed: () => store.reject(s.id), child: const Text('Rejeitar'))),
                        const SizedBox(width: 8),
                        Expanded(child: FilledButton(onPressed: () => store.approve(s.id), child: const Text('Aprovar'))),
                      ]),
                    ]),
                  ),
                ),
              const SizedBox(height: 8),
            ],
            const SectionTitle('Sessões'),
            if (board.sessions.isEmpty)
              Panel(child: Text('Nenhuma sessão nos últimos minutos.', style: TextStyle(color: c.muted)))
            else
              Panel(
                padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
                child: Column(children: [
                  for (final (i, s) in board.sessions.indexed)
                    Container(
                      padding: const EdgeInsets.symmetric(vertical: 12),
                      decoration: BoxDecoration(border: i == 0 ? null : Border(top: BorderSide(color: c.lineSoft))),
                      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
                        Padding(padding: const EdgeInsets.only(top: 6), child: StatusDot(s.activity)),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                            Text.rich(TextSpan(children: [
                              TextSpan(text: s.agent, style: const TextStyle(fontWeight: FontWeight.w600)),
                              TextSpan(
                                text: ' ${s.activityLabel}',
                                style: TextStyle(color: switch (s.activity) { 'working' => c.accent, 'waiting' => c.attention, _ => c.muted }),
                              ),
                            ]), style: const TextStyle(fontSize: 15)),
                            const SizedBox(height: 3),
                            Text(
                              [
                                s.place,
                                if (s.taskId != null && s.taskTitle != null) 'em #${s.taskId} ${s.taskTitle}',
                                s.activity == 'possibly_stopped' ? 'último sinal ${s.lastSeenLabel}' : s.lastSeenLabel,
                              ].join(' · '),
                              style: TextStyle(fontSize: 13, color: c.muted),
                            ),
                            if (s.remoteUrl != null && s.activity != 'ended')
                              TextButton.icon(
                                style: TextButton.styleFrom(padding: EdgeInsets.zero, visualDensity: VisualDensity.compact),
                                onPressed: () => openRemote(context, s.remoteUrl!),
                                icon: const Icon(Icons.open_in_new, size: 15),
                                label: const Text('Abrir no Claude'),
                              ),
                          ]),
                        ),
                      ]),
                    ),
                ]),
              ),
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text('O celular vê as sessões do PC a cada sincronização.', style: TextStyle(fontSize: 12, color: c.muted)),
            ),
          ]),
        ),
      ),
    ]);
  }
}

/// Abre a sessão no controle remoto nativo do agente. O link do claude.ai abre o app do Claude
/// quando ele está instalado; senão, o navegador.
Future<void> openRemote(BuildContext context, String url) async {
  final messenger = ScaffoldMessenger.of(context);
  final ok = await launchUrl(Uri.parse(url), mode: LaunchMode.externalApplication).catchError((_) => false);
  if (!ok) messenger.showSnackBar(const SnackBar(content: Text('Não consegui abrir o link.')));
}

// ---------- Mais ----------

class MoreScreen extends StatelessWidget {
  final ArcaStore store;
  const MoreScreen({super.key, required this.store});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      const ScreenHeader('Mais'),
      Expanded(
        child: ListView(padding: const EdgeInsets.fromLTRB(16, 4, 16, 24), children: [
          const SectionTitle('Sincronização'),
          for (final peer in store.peers) ...[
            PeerCard(store: store, peer: peer),
            const SizedBox(height: 8),
          ],
          if (store.peers.isEmpty)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: Text('Ainda não pareado. O PC é o servidor: abra o Arca4agents nele e leia o QR code.', style: TextStyle(color: c.text2, height: 1.4)),
            ),
          OutlinedButton.icon(
            onPressed: () => openPair(context, store),
            icon: const Icon(Icons.qr_code_scanner, size: 20),
            label: Text(store.peers.isEmpty ? 'Parear com o PC' : 'Parear de novo'),
          ),
          const SizedBox(height: 22),
          const SectionTitle('Tarefas mostradas'),
          SegmentedButton<String>(
            showSelectedIcon: false,
            segments: const [
              ButtonSegment(value: 'user', label: Text('Minhas')),
              ButtonSegment(value: 'all', label: Text('+ agentes')),
              ButtonSegment(value: 'agents', label: Text('Só agentes')),
            ],
            selected: {store.authors},
            onSelectionChanged: (s) => store.setAuthors(s.first),
          ),
          const SizedBox(height: 22),
          Panel(
            padding: EdgeInsets.zero,
            child: Column(children: [
              ListTile(
                leading: const Icon(Icons.folder_outlined),
                title: const Text('Projetos'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => openProjects(context, store),
              ),
              Divider(height: 1, color: c.lineSoft),
              ListTile(
                leading: const Icon(Icons.history),
                title: const Text('Histórico'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => openHistory(context, store),
              ),
              Divider(height: 1, color: c.lineSoft),
              ListTile(
                leading: const Icon(Icons.bar_chart),
                title: const Text('Gráficos'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => openStats(context, store),
              ),
            ]),
          ),
          const SizedBox(height: 22),
          FutureBuilder<PackageInfo>(
            future: PackageInfo.fromPlatform(),
            builder: (context, snap) => Text(
              snap.hasData ? 'Arca4agents ${snap.data!.version}' : '',
              textAlign: TextAlign.center,
              style: TextStyle(fontSize: 12, color: c.muted),
            ),
          ),
        ]),
      ),
    ]);
  }
}

class PeerCard extends StatelessWidget {
  final ArcaStore store;
  final Peer peer;
  const PeerCard({super.key, required this.store, required this.peer});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Panel(
      padding: const EdgeInsets.fromLTRB(16, 10, 8, 10),
      child: Row(children: [
        Icon(Icons.desktop_windows_outlined, color: c.text2),
        const SizedBox(width: 12),
        Expanded(
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(peer.name, style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w500)),
            Text(store.lastSyncOk == false ? 'sem conexão agora · ${peer.lastSyncedLabel}' : peer.lastSyncedLabel, style: TextStyle(fontSize: 13, color: c.muted)),
          ]),
        ),
        TextButton(
          style: TextButton.styleFrom(backgroundColor: c.accentTint, foregroundColor: c.accent, minimumSize: const Size(44, 44)),
          onPressed: store.syncing ? null : () => store.sync(),
          child: Text(store.syncing ? '…' : 'Agora', style: const TextStyle(fontWeight: FontWeight.w600)),
        ),
        PopupMenuButton<String>(
          tooltip: 'Opções',
          onSelected: (_) => store.unpair(peer.device),
          itemBuilder: (_) => [const PopupMenuItem(value: 'unpair', child: Text('Desparear'))],
        ),
      ]),
    );
  }
}

// ---------- Pareamento ----------

void openPair(BuildContext context, ArcaStore store) {
  Navigator.push(context, MaterialPageRoute(builder: (_) => PairScreen(store: store)));
}

class PairScreen extends StatefulWidget {
  final ArcaStore store;
  const PairScreen({super.key, required this.store});

  @override
  State<PairScreen> createState() => _PairScreenState();
}

class _PairScreenState extends State<PairScreen> {
  bool _busy = false;

  Future<void> _pair(String code) async {
    if (_busy) return;
    setState(() => _busy = true);
    final ok = await widget.store.pair(code);
    if (!mounted) return;
    setState(() => _busy = false);
    if (ok) {
      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Pareado. A sincronização agora é automática.')));
      Navigator.pop(context);
    }
  }

  Future<void> _paste() async {
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
    if (ok == true) await _pair(code.text);
  }

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final store = widget.store;
    return Scaffold(
      body: SafeArea(
        child: ListenableBuilder(
          listenable: store,
          builder: (context, _) => Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            const ScreenHeader('Parear com o PC', back: true),
            Expanded(
              child: ListView(padding: const EdgeInsets.fromLTRB(16, 8, 16, 24), children: [
                ClipRRect(
                  borderRadius: BorderRadius.circular(20),
                  child: Container(
                    height: 340,
                    color: const Color(0xFF1B1F1D),
                    child: Stack(alignment: Alignment.center, children: [
                      if (Platform.isAndroid && !_busy)
                        MobileScanner(
                          onDetect: (capture) {
                            final value = capture.barcodes.map((b) => b.rawValue).whereType<String>().where((v) => v.contains('"arca"')).firstOrNull;
                            if (value != null) _pair(value);
                          },
                        )
                      else if (!_busy)
                        const Text('A câmera só funciona no celular.', style: TextStyle(color: Color(0xFFE6E8E3))),
                      if (_busy) const CircularProgressIndicator(color: Colors.white),
                      const IgnorePointer(child: SizedBox(width: 210, height: 210, child: CustomPaint(painter: _ViewfinderPainter()))),
                    ]),
                  ),
                ),
                const SizedBox(height: 16),
                Text.rich(
                  TextSpan(children: [
                    const TextSpan(text: 'No PC, abra o Arca4agents › '),
                    TextSpan(text: 'Sincronização', style: TextStyle(fontWeight: FontWeight.w600, color: c.text)),
                    const TextSpan(text: ' e aponte a câmera para o QR code. Os dois precisam estar no mesmo Wi-Fi.'),
                  ]),
                  style: TextStyle(fontSize: 15, height: 1.45, color: c.text2),
                ),
                const SizedBox(height: 16),
                OutlinedButton.icon(onPressed: _busy ? null : _paste, icon: const Icon(Icons.content_paste, size: 18), label: const Text('Colar código')),
                if (store.error != null)
                  Padding(padding: const EdgeInsets.only(top: 12), child: Text(store.error!, style: TextStyle(color: c.attention))),
                if (store.peers.isNotEmpty) ...[
                  const SizedBox(height: 24),
                  const SectionTitle('Pareado com'),
                  for (final peer in store.peers) PeerCard(store: store, peer: peer),
                ],
              ]),
            ),
          ]),
        ),
      ),
    );
  }
}

/// Os quatro cantos da mira da câmera.
class _ViewfinderPainter extends CustomPainter {
  const _ViewfinderPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = Colors.white
      ..strokeWidth = 4
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round;
    const arm = 34.0;
    final w = size.width, h = size.height;
    for (final (x, y, dx, dy) in [(0.0, 0.0, 1.0, 1.0), (w, 0.0, -1.0, 1.0), (0.0, h, 1.0, -1.0), (w, h, -1.0, -1.0)]) {
      canvas.drawPath(
        Path()
          ..moveTo(x, y + dy * arm)
          ..lineTo(x, y)
          ..lineTo(x + dx * arm, y),
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

// ---------- Histórico e projetos ----------

void openHistory(BuildContext context, ArcaStore store) {
  Navigator.push(context, MaterialPageRoute(builder: (_) => HistoryScreen(store: store)));
}

/// Tag com o tom do projeto, usada no histórico.
class ProjectPill extends StatelessWidget {
  final String name;
  const ProjectPill(this.name, {super.key});

  @override
  Widget build(BuildContext context) {
    final color = projectColor(name);
    return Container(
      padding: const EdgeInsets.fromLTRB(7, 1, 8, 1),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        border: Border.all(color: color.withValues(alpha: 0.35)),
        borderRadius: BorderRadius.circular(99),
      ),
      child: Row(mainAxisSize: MainAxisSize.min, children: [
        ProjectDot(name, size: 7),
        const SizedBox(width: 5),
        Text(name, style: TextStyle(fontSize: 11, fontWeight: FontWeight.w500, color: context.arca.text2)),
      ]),
    );
  }
}

class HistoryScreen extends StatefulWidget {
  final ArcaStore store;
  const HistoryScreen({super.key, required this.store});

  @override
  State<HistoryScreen> createState() => _HistoryScreenState();
}

class _HistoryScreenState extends State<HistoryScreen> {
  static const _pageSize = 20;
  late final Future<List<HistoryView>> _items = widget.store.history();
  int _page = 0;

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Scaffold(
      body: SafeArea(
        child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          const ScreenHeader('Histórico', back: true),
          Expanded(
            child: FutureBuilder<List<HistoryView>>(
              future: _items,
              builder: (context, snap) {
                if (!snap.hasData) return const Center(child: CircularProgressIndicator());
                final items = snap.data!;
                if (items.isEmpty) return Center(child: Text('Nada concluído ainda.', style: TextStyle(color: c.muted)));
                final pages = (items.length / _pageSize).ceil();
                final page = _page.clamp(0, pages - 1);
                final shown = items.skip(page * _pageSize).take(_pageSize).toList();
                return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                  Expanded(
                    child: ListView.builder(
                      // A chave por página volta a lista para o topo ao trocar de página.
                      key: PageStorageKey('history-$page'),
                      padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
                      itemCount: shown.length,
                      itemBuilder: (_, i) {
                        final h = shown[i];
                        final date = h.date.substring(0, 10).split('-').reversed.join('/');
                        return Container(
                          padding: const EdgeInsets.symmetric(vertical: 12),
                          decoration: BoxDecoration(border: Border(bottom: BorderSide(color: c.line))),
                          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                            Text(h.title, style: const TextStyle(fontSize: 15)),
                            const SizedBox(height: 5),
                            Wrap(spacing: 8, runSpacing: 4, crossAxisAlignment: WrapCrossAlignment.center, children: [
                              Text(date, style: TextStyle(fontSize: 12, color: c.muted, fontFamily: mono)),
                              if (h.project != null) ProjectPill(h.project!),
                              if (h.daily) Text('diária', style: TextStyle(fontSize: 12, color: c.muted)),
                              if (h.agent != null) AgentChip(h.agent!),
                            ]),
                          ]),
                        );
                      },
                    ),
                  ),
                  if (pages > 1)
                    Container(
                      padding: const EdgeInsets.fromLTRB(8, 6, 8, 6),
                      decoration: BoxDecoration(color: c.surface, border: Border(top: BorderSide(color: c.line))),
                      child: Row(children: [
                        IconButton(tooltip: 'Primeira página', onPressed: page > 0 ? () => setState(() => _page = 0) : null, icon: const Icon(Icons.first_page)),
                        IconButton(tooltip: 'Anterior', onPressed: page > 0 ? () => setState(() => _page = page - 1) : null, icon: const Icon(Icons.chevron_left)),
                        Expanded(
                          child: Text(
                            '${page * _pageSize + 1}–${(page + 1) * _pageSize > items.length ? items.length : (page + 1) * _pageSize} de ${items.length}',
                            textAlign: TextAlign.center,
                            style: TextStyle(fontSize: 13, color: c.muted, fontFamily: mono),
                          ),
                        ),
                        IconButton(tooltip: 'Próxima', onPressed: page < pages - 1 ? () => setState(() => _page = page + 1) : null, icon: const Icon(Icons.chevron_right)),
                        IconButton(tooltip: 'Última página', onPressed: page < pages - 1 ? () => setState(() => _page = pages - 1) : null, icon: const Icon(Icons.last_page)),
                      ]),
                    ),
                ]);
              },
            ),
          ),
        ]),
      ),
    );
  }
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
          final c = context.arca;
          final projects = store.board?.projects ?? [];
          return Scaffold(
            floatingActionButton: FloatingActionButton(
              tooltip: 'Novo projeto',
              onPressed: () async {
                final name = TextEditingController();
                final ok = await showDialog<bool>(
                  context: context,
                  builder: (context) => AlertDialog(
                    title: const Text('Novo projeto'),
                    content: TextField(controller: name, autofocus: true, decoration: const InputDecoration(hintText: 'Nome')),
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
            body: SafeArea(
              child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                const ScreenHeader('Projetos', back: true),
                Expanded(
                  child: projects.isEmpty
                      ? Center(child: Text('Nenhum projeto. As pastas vinculadas ficam no PC.', style: TextStyle(color: c.muted)))
                      : ListView(padding: const EdgeInsets.fromLTRB(16, 0, 16, 96), children: [
                          for (final p in projects)
                            Padding(
                              padding: const EdgeInsets.only(bottom: 10),
                              child: Panel(
                                padding: const EdgeInsets.fromLTRB(16, 12, 4, 12),
                                child: Row(children: [
                                  ProjectDot(p.name, size: 10),
                                  const SizedBox(width: 12),
                                  Expanded(
                                    child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                                      Text(p.name, style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w500)),
                                      const SizedBox(height: 2),
                                      Text('estante ${p.shelf} · carrinho ${p.cart} · feitas ${p.done}', style: TextStyle(fontSize: 13, color: c.muted)),
                                      Text(_policies[p.policy] ?? p.policy, style: TextStyle(fontSize: 13, color: c.muted)),
                                    ]),
                                  ),
                                  PopupMenuButton<String>(
                                    tooltip: 'Opções',
                                    onSelected: (v) => v == 'delete' ? store.deleteProject(p.id) : store.setPolicy(p.id, v),
                                    itemBuilder: (_) => [
                                      for (final e in _policies.entries) CheckedPopupMenuItem(value: e.key, checked: p.policy == e.key, child: Text(e.value)),
                                      const PopupMenuDivider(),
                                      const PopupMenuItem(value: 'delete', child: Text('Apagar (as tarefas ficam)')),
                                    ],
                                  ),
                                ]),
                              ),
                            ),
                        ]),
                ),
              ]),
            ),
          );
        },
      )));
}

// ---------- Datas e Aniversários ----------

Color _birthdayColor(BuildContext context) =>
    Theme.of(context).brightness == Brightness.dark ? const Color(0xFFF48FB1) : const Color(0xFFB83280);

Color _upcomingDueColor(BuildContext context, String dueState) {
  final c = context.arca;
  return switch (dueState) {
    'overdue' => Theme.of(context).colorScheme.error,
    'today' => c.attention,
    _ => c.muted,
  };
}

class _UpcomingStrip extends StatelessWidget {
  final List<UpcomingItemView> items;
  final VoidCallback onTap;

  const _UpcomingStrip({required this.items, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final hasUrgent = items.any((i) => i.dueState == 'overdue' || i.dueState == 'today');

    return Material(
      color: c.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: BorderSide(color: hasUrgent ? c.attentionDot.withValues(alpha: 0.45) : c.line),
      ),
      child: InkWell(
        borderRadius: BorderRadius.circular(12),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 10, 8, 10),
          child: Row(children: [
            Icon(Icons.event_outlined, size: 17, color: hasUrgent ? c.attention : c.accent),
            const SizedBox(width: 8),
            Expanded(
              child: LayoutBuilder(builder: (context, constraints) {
                final textDirection = Directionality.of(context);
                final textScaler = MediaQuery.textScalerOf(context);
                final baseStyle = DefaultTextStyle.of(context).style.merge(const TextStyle(fontSize: 12));
                final candidates = items.take(4).toList();
                const gap = 6.0;

                double measureChip(UpcomingItemView item) {
                  final tp = TextPainter(
                    text: TextSpan(
                      style: baseStyle,
                      children: [
                        TextSpan(text: item.title, style: const TextStyle(fontWeight: FontWeight.w500)),
                        TextSpan(
                          text: ' · ${item.dueLabel}',
                          style: TextStyle(
                            fontWeight: item.kind == 'birthday' && item.dueState == 'today' ? FontWeight.w700 : null,
                          ),
                        ),
                      ],
                    ),
                    textDirection: textDirection,
                    textScaler: textScaler,
                    maxLines: 1,
                  )..layout();
                  final iconAndBorder = item.kind == 'birthday' ? 20.0 : 0.0;
                  return (tp.width + 20.0 + iconAndBorder).clamp(0.0, _UpcomingStripChip.maxWidth);
                }

                double measureMore(int count) {
                  final tp = TextPainter(
                    text: TextSpan(
                      text: '+$count',
                      style: baseStyle.copyWith(color: c.muted, fontWeight: FontWeight.w600),
                    ),
                    textDirection: textDirection,
                    textScaler: textScaler,
                    maxLines: 1,
                  )..layout();
                  return tp.width;
                }

                final widths = [for (final item in candidates) measureChip(item)];
                var fitCount = 0;
                for (var k = candidates.length; k >= 0; k--) {
                  var total = 0.0;
                  for (var i = 0; i < k; i++) {
                    total += widths[i];
                  }
                  if (k > 1) total += (k - 1) * gap;
                  final remaining = items.length - k;
                  if (remaining > 0) {
                    total += (k > 0 ? gap : 0) + measureMore(remaining);
                  }
                  if (total <= constraints.maxWidth) {
                    fitCount = k;
                    break;
                  }
                }

                final visible = candidates.take(fitCount).toList();
                final restCount = items.length - visible.length;

                return Row(children: [
                  for (var i = 0; i < visible.length; i++) ...[
                    if (i > 0) const SizedBox(width: gap),
                    _UpcomingStripChip(item: visible[i]),
                  ],
                  if (restCount > 0) ...[
                    if (visible.isNotEmpty) const SizedBox(width: gap),
                    Flexible(
                      child: Text(
                        '+$restCount',
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(fontSize: 12, color: c.muted, fontWeight: FontWeight.w600),
                      ),
                    ),
                  ],
                ]);
              }),
            ),
            Icon(Icons.chevron_right, size: 18, color: c.muted),
          ]),
        ),
      ),
    );
  }
}

class _UpcomingStripChip extends StatelessWidget {
  static const double maxWidth = 200.0;
  final UpcomingItemView item;

  const _UpcomingStripChip({required this.item});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final isBirthday = item.kind == 'birthday';
    final bdayColor = _birthdayColor(context);
    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: maxWidth),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 4),
        decoration: BoxDecoration(
          color: isBirthday
              ? bdayColor.withValues(alpha: item.dueState == 'today' ? 0.22 : 0.14)
              : item.dueState == 'overdue'
                  ? Theme.of(context).colorScheme.error.withValues(alpha: 0.1)
                  : item.dueState == 'today'
                      ? c.attentionTint
                      : c.recessed,
          borderRadius: BorderRadius.circular(999),
          border: isBirthday ? Border.all(color: bdayColor.withValues(alpha: 0.35)) : null,
        ),
        child: Row(mainAxisSize: MainAxisSize.min, children: [
          if (isBirthday) ...[
            Icon(Icons.celebration_outlined, size: 14, color: bdayColor),
            const SizedBox(width: 4),
          ],
          Flexible(
            child: Text.rich(
              TextSpan(children: [
                TextSpan(
                  text: item.title,
                  style: TextStyle(fontWeight: FontWeight.w500, color: isBirthday ? bdayColor : c.text),
                ),
                TextSpan(
                  text: ' · ${item.dueLabel}',
                  style: TextStyle(
                    fontWeight: isBirthday && item.dueState == 'today' ? FontWeight.w700 : null,
                    color: isBirthday ? bdayColor : _upcomingDueColor(context, item.dueState),
                  ),
                ),
              ]),
              style: const TextStyle(fontSize: 12),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ]),
      ),
    );
  }
}

class DatesScreen extends StatefulWidget {
  final ArcaStore store;
  final Board board;
  final String tab;
  final ValueChanged<String> onTab;

  const DatesScreen({
    super.key,
    required this.store,
    required this.board,
    required this.tab,
    required this.onTab,
  });

  @override
  State<DatesScreen> createState() => _DatesScreenState();
}

class _DatesScreenState extends State<DatesScreen> {
  int? _horizonDays;
  String _kindFilter = 'all';
  String _groupFilter = '';
  bool _byDate = false;

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final overview = widget.store.dates;

    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Padding(
        padding: const EdgeInsets.fromLTRB(16, 10, 6, 10),
        child: Row(children: [
          const Text('Datas', style: TextStyle(fontSize: 19, fontWeight: FontWeight.w700)),
          const Spacer(),
          _SyncButton(store: widget.store),
        ]),
      ),
      Padding(
        padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
        child: Container(
          padding: const EdgeInsets.all(4),
          decoration: BoxDecoration(color: c.recessed, borderRadius: BorderRadius.circular(12)),
          child: Row(children: [
            for (final (key, label) in [('upcoming', 'Próximas datas'), ('birthdays', 'Aniversários')])
              Expanded(
                child: GestureDetector(
                  onTap: () => widget.onTab(key),
                  child: AnimatedContainer(
                    duration: const Duration(milliseconds: 150),
                    height: 38,
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: widget.tab == key ? c.surface : Colors.transparent,
                      borderRadius: BorderRadius.circular(9),
                      boxShadow: widget.tab == key
                          ? [BoxShadow(color: Colors.black.withValues(alpha: 0.06), blurRadius: 2, offset: const Offset(0, 1))]
                          : null,
                    ),
                    child: Text(
                      label,
                      style: TextStyle(
                        fontSize: 13,
                        fontWeight: widget.tab == key ? FontWeight.w600 : FontWeight.w500,
                        color: widget.tab == key ? c.text : c.muted,
                      ),
                    ),
                  ),
                ),
              ),
          ]),
        ),
      ),
      Expanded(
        child: overview == null
            ? const Center(child: CircularProgressIndicator())
            : widget.tab == 'upcoming'
                ? _buildUpcoming(context, overview)
                : _buildBirthdays(context, overview),
      ),
    ]);
  }

  Widget _buildUpcoming(BuildContext context, DatesOverview overview) {
    final c = context.arca;
    final filtered = overview.upcoming.where((item) {
      if (_horizonDays != null && item.daysUntil > _horizonDays!) return false;
      if (_kindFilter != 'all' && item.kind != _kindFilter) return false;
      return true;
    }).toList();

    final buckets = <(String, bool Function(int))>[
      ('ATRASADOS', (d) => d < 0),
      ('HOJE', (d) => d == 0),
      ('PRÓXIMOS 7 DIAS', (d) => d >= 1 && d <= 7),
      ('ESTE MÊS', (d) => d >= 8 && d <= 30),
      ('MAIS ADIANTE', (d) => d > 30),
    ];

    final sections = [
      for (final (title, pred) in buckets)
        if (filtered.any((i) => pred(i.daysUntil)))
          (title, filtered.where((i) => pred(i.daysUntil)).toList()),
    ];

    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 0, 16, 96),
      children: [
        SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Row(children: [
            for (final (days, label) in [(null, 'Tudo'), (7, 'Até 7d'), (30, 'Até 30d'), (90, 'Até 90d')]) ...[
              _SmallChip(
                label: label,
                selected: _horizonDays == days,
                onTap: () => setState(() => _horizonDays = days),
              ),
              const SizedBox(width: 6),
            ],
            const SizedBox(width: 6),
            for (final (kind, label) in [('all', 'Todos'), ('event', 'Eventos'), ('task', 'Tarefas'), ('birthday', 'Aniv.')]) ...[
              _SmallChip(
                label: label,
                selected: _kindFilter == kind,
                onTap: () => setState(() => _kindFilter = kind),
              ),
              const SizedBox(width: 6),
            ],
          ]),
        ),
        const SizedBox(height: 12),
        if (sections.isEmpty)
          Panel(
            padding: const EdgeInsets.all(20),
            child: Text(
              'Nenhuma data próxima nesse filtro. Use + para cadastrar um evento.',
              textAlign: TextAlign.center,
              style: TextStyle(color: c.muted, fontSize: 13),
            ),
          )
        else
          for (final (title, items) in sections) ...[
            Padding(
              padding: const EdgeInsets.only(top: 6, bottom: 6, left: 4),
              child: Text(
                '$title (${items.length})',
                style: TextStyle(fontSize: 11, fontWeight: FontWeight.w700, letterSpacing: 0.6, color: c.muted),
              ),
            ),
            Panel(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Column(children: [
                for (var i = 0; i < items.length; i++) ...[
                  if (i > 0) Divider(height: 1, color: c.lineSoft),
                  _UpcomingRow(
                    item: items[i],
                    onTap: () {
                      final item = items[i];
                      if (item.kind == 'event') {
                        showTrackedDateSheet(context, widget.store, existing: item);
                      } else if (item.kind == 'birthday') {
                        final b = overview.birthdays.where((x) => x.id == item.id).firstOrNull;
                        if (b != null) showBirthdaySheet(context, widget.store, existing: b);
                      }
                    },
                    onCompleteTask: items[i].kind == 'task'
                        ? () => widget.store.moveTask(items[i].id, 'DONE')
                        : null,
                  ),
                ],
              ]),
            ),
            const SizedBox(height: 8),
          ],
      ],
    );
  }

  Widget _buildBirthdays(BuildContext context, DatesOverview overview) {
    final c = context.arca;
    final counts = <String, int>{};
    for (final b in overview.birthdays) {
      counts[b.group] = (counts[b.group] ?? 0) + 1;
    }
    final groups = counts.entries.toList()..sort((a, b) => a.key.compareTo(b.key));
    final visible = _groupFilter.isEmpty
        ? overview.birthdays
        : overview.birthdays.where((b) => b.group == _groupFilter).toList();

    final byGroup = <String, List<BirthdayView>>{};
    for (final b in visible) {
      byGroup.putIfAbsent(b.group, () => []).add(b);
    }
    final groupSections = byGroup.entries.toList()..sort((a, b) => a.key.compareTo(b.key));

    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 0, 16, 96),
      children: [
        SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Row(children: [
            _SmallChip(
              label: 'Todos (${overview.birthdays.length})',
              selected: _groupFilter.isEmpty,
              onTap: () => setState(() => _groupFilter = ''),
            ),
            for (final entry in groups) ...[
              const SizedBox(width: 6),
              _SmallChip(
                label: '${entry.key} (${entry.value})',
                selected: _groupFilter == entry.key,
                onTap: () => setState(() => _groupFilter = entry.key),
              ),
            ],
          ]),
        ),
        const SizedBox(height: 10),
        Row(children: [
          _SmallChip(
            label: 'Por grupo',
            selected: !_byDate,
            onTap: () => setState(() => _byDate = false),
          ),
          const SizedBox(width: 6),
          _SmallChip(
            label: 'Por proximidade',
            selected: _byDate,
            onTap: () => setState(() => _byDate = true),
          ),
        ]),
        const SizedBox(height: 12),
        if (visible.isEmpty)
          Panel(
            padding: const EdgeInsets.all(20),
            child: Text(
              'Nenhum aniversário cadastrado. Use + para adicionar.',
              textAlign: TextAlign.center,
              style: TextStyle(color: c.muted, fontSize: 13),
            ),
          )
        else if (_byDate)
          Panel(
            padding: const EdgeInsets.symmetric(vertical: 4),
            child: Column(children: [
              for (var i = 0; i < visible.length; i++) ...[
                if (i > 0) Divider(height: 1, color: c.lineSoft),
                _BirthdayRow(
                  item: visible[i],
                  showGroup: true,
                  onTap: () => showBirthdaySheet(context, widget.store, existing: visible[i]),
                ),
              ],
            ]),
          )
        else
          for (final entry in groupSections) ...[
            Padding(
              padding: const EdgeInsets.only(top: 6, bottom: 6, left: 4),
              child: Text(
                '${entry.key.toUpperCase()} (${entry.value.length})',
                style: TextStyle(fontSize: 11, fontWeight: FontWeight.w700, letterSpacing: 0.6, color: c.muted),
              ),
            ),
            Panel(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Column(children: [
                for (var i = 0; i < entry.value.length; i++) ...[
                  if (i > 0) Divider(height: 1, color: c.lineSoft),
                  _BirthdayRow(
                    item: entry.value[i],
                    showGroup: false,
                    onTap: () => showBirthdaySheet(context, widget.store, existing: entry.value[i]),
                  ),
                ],
              ]),
            ),
            const SizedBox(height: 8),
          ],
      ],
    );
  }
}

class _SmallChip extends StatelessWidget {
  final String label;
  final bool selected;
  final VoidCallback onTap;

  const _SmallChip({required this.label, required this.selected, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return GestureDetector(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 11, vertical: 6),
        decoration: BoxDecoration(
          color: selected ? c.accentTint : c.surface,
          borderRadius: BorderRadius.circular(999),
          border: Border.all(color: selected ? c.accentLine : c.line),
        ),
        child: Text(
          label,
          style: TextStyle(
            fontSize: 12,
            fontWeight: selected ? FontWeight.w600 : FontWeight.w500,
            color: selected ? c.accent : c.muted,
          ),
        ),
      ),
    );
  }
}

class _UpcomingRow extends StatelessWidget {
  final UpcomingItemView item;
  final VoidCallback onTap;
  final VoidCallback? onCompleteTask;

  const _UpcomingRow({required this.item, required this.onTap, this.onCompleteTask});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final bdayColor = _birthdayColor(context);
    final (badgeText, badgeColor, badgeBg) = switch (item.kind) {
      'event' => ('Evento', c.agent, c.agentTint),
      'task' => ('Tarefa', c.accent, c.accentTint),
      _ => ('Aniv.', bdayColor, bdayColor.withValues(alpha: 0.14)),
    };

    return InkWell(
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(14, 11, 10, 11),
        child: Row(children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
            decoration: BoxDecoration(color: badgeBg, borderRadius: BorderRadius.circular(6)),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (item.kind == 'birthday') ...[
                  Icon(Icons.celebration_outlined, size: 13, color: badgeColor),
                  const SizedBox(width: 4),
                ],
                Text(badgeText, style: TextStyle(fontSize: 11, fontWeight: FontWeight.w600, color: badgeColor)),
              ],
            ),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(item.title, style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500)),
              if (item.project != null || item.group != null || item.turningAge != null || item.note != null) ...[
                const SizedBox(height: 3),
                Wrap(spacing: 8, runSpacing: 2, crossAxisAlignment: WrapCrossAlignment.center, children: [
                  if (item.project != null)
                    Row(mainAxisSize: MainAxisSize.min, children: [
                      ProjectDot(item.project!, size: 7),
                      const SizedBox(width: 4),
                      Text(item.project!, style: TextStyle(fontSize: 12, color: c.muted)),
                    ]),
                  if (item.group != null) Text(item.group!, style: TextStyle(fontSize: 12, color: c.muted)),
                  if (item.turningAge != null)
                    Text('faz ${item.turningAge} anos', style: TextStyle(fontSize: 12, color: c.muted)),
                  if (item.note != null) Text(item.note!, style: TextStyle(fontSize: 12, color: c.muted)),
                ]),
              ],
            ]),
          ),
          const SizedBox(width: 8),
          Column(crossAxisAlignment: CrossAxisAlignment.end, children: [
            Text(item.dateLabel, style: TextStyle(fontSize: 12, fontFamily: mono, color: c.muted)),
            Text(
              item.dueLabel,
              style: TextStyle(
                fontSize: 12,
                fontWeight: item.dueState == 'overdue' || item.dueState == 'today' ? FontWeight.w600 : FontWeight.w400,
                color: _upcomingDueColor(context, item.dueState),
              ),
            ),
          ]),
          if (onCompleteTask != null) ...[
            const SizedBox(width: 4),
            IconButton(
              tooltip: 'Concluir tarefa',
              onPressed: onCompleteTask,
              icon: Icon(Icons.check_circle_outline, size: 20, color: c.accent),
            ),
          ],
        ]),
      ),
    );
  }
}

class _BirthdayRow extends StatelessWidget {
  final BirthdayView item;
  final bool showGroup;
  final VoidCallback onTap;

  const _BirthdayRow({required this.item, required this.showGroup, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final bdayColor = _birthdayColor(context);
    return InkWell(
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(14, 11, 14, 11),
        child: Row(children: [
          Container(
            width: 28,
            height: 28,
            decoration: BoxDecoration(
              color: bdayColor.withValues(alpha: 0.14),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Icon(Icons.celebration_outlined, size: 16, color: bdayColor),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Row(children: [
                Flexible(
                  child: Text(item.name, style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500)),
                ),
                if (showGroup) ...[
                  const SizedBox(width: 8),
                  Text(item.group, style: TextStyle(fontSize: 12, color: c.muted)),
                ],
              ]),
              if (item.turningAge != null || item.note != null) ...[
                const SizedBox(height: 2),
                Text(
                  [
                    if (item.turningAge != null) 'faz ${item.turningAge} anos',
                    if (item.note != null) item.note!,
                  ].join(' · '),
                  style: TextStyle(fontSize: 12, color: c.muted),
                ),
              ],
            ]),
          ),
          const SizedBox(width: 8),
          Column(crossAxisAlignment: CrossAxisAlignment.end, children: [
            Text(item.dateLabel, style: TextStyle(fontSize: 12, fontFamily: mono, color: c.muted)),
            Text(
              item.dueLabel,
              style: TextStyle(
                fontSize: 12,
                fontWeight: item.dueState == 'today' ? FontWeight.w600 : FontWeight.w400,
                color: _upcomingDueColor(context, item.dueState),
              ),
            ),
          ]),
        ]),
      ),
    );
  }
}

({int month, int day, int? year})? _parseBirthdayDate(String raw) {
  final s = raw.trim();
  final dmy = RegExp(r'^(\d{1,2})/(\d{1,2})(?:/(\d{4}))?$').firstMatch(s);
  if (dmy != null) {
    return (
      day: int.parse(dmy.group(1)!),
      month: int.parse(dmy.group(2)!),
      year: dmy.group(3) != null ? int.parse(dmy.group(3)!) : null,
    );
  }
  final ymd = RegExp(r'^(\d{4})-(\d{1,2})-(\d{1,2})$').firstMatch(s);
  if (ymd != null) {
    return (
      year: int.parse(ymd.group(1)!),
      month: int.parse(ymd.group(2)!),
      day: int.parse(ymd.group(3)!),
    );
  }
  return null;
}

Future<void> showBirthdaySheet(BuildContext context, ArcaStore store, {BirthdayView? existing}) {
  final nameCtrl = TextEditingController(text: existing?.name ?? '');
  final dateCtrl = TextEditingController(text: existing?.dateLabel ?? '');
  final groupCtrl = TextEditingController(text: existing?.group ?? '');
  final noteCtrl = TextEditingController(text: existing?.note ?? '');
  final knownGroups = (store.dates?.birthdays.map((b) => b.group).toSet().toList() ?? <String>[])..sort();

  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    builder: (ctx) {
      String? localError;
      bool busy = false;
      return StatefulBuilder(builder: (ctx, setSheetState) {
        final c = ctx.arca;
        return Padding(
          padding: EdgeInsets.fromLTRB(20, 18, 20, MediaQuery.viewInsetsOf(ctx).bottom + 20),
          child: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  existing == null ? 'Novo aniversário' : 'Editar aniversário',
                  style: const TextStyle(fontSize: 17, fontWeight: FontWeight.w700),
                ),
                const SizedBox(height: 12),
                if (localError != null) ...[
                  Text(localError!, style: TextStyle(color: Theme.of(ctx).colorScheme.error, fontSize: 13)),
                  const SizedBox(height: 8),
                ],
                TextField(
                  controller: nameCtrl,
                  autofocus: existing == null,
                  decoration: const InputDecoration(labelText: 'Nome'),
                ),
                const SizedBox(height: 10),
                TextField(
                  controller: dateCtrl,
                  keyboardType: TextInputType.datetime,
                  decoration: const InputDecoration(
                    labelText: 'Data',
                    hintText: 'DD/MM ou DD/MM/AAAA',
                  ),
                ),
                const SizedBox(height: 10),
                TextField(
                  controller: groupCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Grupo',
                    hintText: 'Geral (ou Família, Amigos, Trabalho…)',
                  ),
                ),
                if (knownGroups.isNotEmpty) ...[
                  const SizedBox(height: 8),
                  Wrap(spacing: 6, runSpacing: 6, children: [
                    for (final g in knownGroups)
                      _SmallChip(
                        label: g,
                        selected: groupCtrl.text.trim() == g,
                        onTap: () => setSheetState(() => groupCtrl.text = g),
                      ),
                  ]),
                ],
                const SizedBox(height: 10),
                TextField(
                  controller: noteCtrl,
                  decoration: const InputDecoration(labelText: 'Nota (opcional)'),
                ),
                const SizedBox(height: 16),
                Row(children: [
                  if (existing != null)
                    TextButton(
                      onPressed: busy
                          ? null
                          : () async {
                              setSheetState(() {
                                busy = true;
                                localError = null;
                              });
                              final ok = await store.deleteBirthday(existing.id);
                              if (!ctx.mounted) return;
                              if (ok) {
                                Navigator.pop(ctx);
                              } else {
                                setSheetState(() {
                                  busy = false;
                                  localError = store.error;
                                });
                              }
                            },
                      child: Text('Apagar', style: TextStyle(color: Theme.of(ctx).colorScheme.error)),
                    ),
                  const Spacer(),
                  TextButton(onPressed: () => Navigator.pop(ctx), child: const Text('Cancelar')),
                  const SizedBox(width: 8),
                  FilledButton(
                    style: FilledButton.styleFrom(backgroundColor: c.accent),
                    onPressed: busy
                        ? null
                        : () async {
                            final parsed = _parseBirthdayDate(dateCtrl.text);
                            if (parsed == null) {
                              setSheetState(() => localError = 'Use o formato DD/MM ou DD/MM/AAAA.');
                              return;
                            }
                            setSheetState(() {
                              busy = true;
                              localError = null;
                            });
                            final ok = existing == null
                                ? await store.addBirthday(
                                    name: nameCtrl.text.trim(),
                                    group: groupCtrl.text.trim().isEmpty ? null : groupCtrl.text.trim(),
                                    month: parsed.month,
                                    day: parsed.day,
                                    year: parsed.year,
                                    note: noteCtrl.text.trim().isEmpty ? null : noteCtrl.text.trim(),
                                  )
                                : await store.editBirthday(
                                    existing.id,
                                    name: nameCtrl.text.trim(),
                                    group: groupCtrl.text.trim(),
                                    month: parsed.month,
                                    day: parsed.day,
                                    year: parsed.year ?? 0,
                                    note: noteCtrl.text.trim(),
                                  );
                            if (!ctx.mounted) return;
                            if (ok) {
                              Navigator.pop(ctx);
                            } else {
                              setSheetState(() {
                                busy = false;
                                localError = store.error;
                              });
                            }
                          },
                    child: const Text('Salvar'),
                  ),
                ]),
              ],
            ),
          ),
        );
      });
    },
  );
}

Future<void> showTrackedDateSheet(BuildContext context, ArcaStore store, {UpcomingItemView? existing}) {
  final titleCtrl = TextEditingController(text: existing?.title ?? '');
  final dateCtrl = TextEditingController(text: existing?.date ?? '');
  final noteCtrl = TextEditingController(text: existing?.note ?? '');
  String selectedProject = existing?.project ?? '';
  final projects = store.board?.projects ?? [];

  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    builder: (ctx) {
      String? localError;
      bool busy = false;
      return StatefulBuilder(builder: (ctx, setSheetState) {
        final c = ctx.arca;
        return Padding(
          padding: EdgeInsets.fromLTRB(20, 18, 20, MediaQuery.viewInsetsOf(ctx).bottom + 20),
          child: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  existing == null ? 'Nova data / evento' : 'Editar data',
                  style: const TextStyle(fontSize: 17, fontWeight: FontWeight.w700),
                ),
                const SizedBox(height: 12),
                if (localError != null) ...[
                  Text(localError!, style: TextStyle(color: Theme.of(ctx).colorScheme.error, fontSize: 13)),
                  const SizedBox(height: 8),
                ],
                TextField(
                  controller: titleCtrl,
                  autofocus: existing == null,
                  decoration: const InputDecoration(labelText: 'Título'),
                ),
                const SizedBox(height: 10),
                TextField(
                  controller: dateCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Data',
                    hintText: 'hoje, amanhã, +7d, DD/MM/AAAA ou AAAA-MM-DD',
                  ),
                ),
                const SizedBox(height: 8),
                Wrap(spacing: 6, children: [
                  for (final preset in ['hoje', 'amanhã', '+7d', '+30d'])
                    _SmallChip(
                      label: preset,
                      selected: dateCtrl.text.trim() == preset,
                      onTap: () => setSheetState(() => dateCtrl.text = preset),
                    ),
                ]),
                if (projects.isNotEmpty) ...[
                  const SizedBox(height: 12),
                  Text('Projeto', style: TextStyle(fontSize: 12, color: c.muted)),
                  const SizedBox(height: 6),
                  Wrap(spacing: 6, runSpacing: 6, children: [
                    _SmallChip(
                      label: 'Sem projeto',
                      selected: selectedProject.isEmpty,
                      onTap: () => setSheetState(() => selectedProject = ''),
                    ),
                    for (final p in projects)
                      _SmallChip(
                        label: p.name,
                        selected: selectedProject == p.name,
                        onTap: () => setSheetState(() => selectedProject = p.name),
                      ),
                  ]),
                ],
                const SizedBox(height: 10),
                TextField(
                  controller: noteCtrl,
                  decoration: const InputDecoration(labelText: 'Nota (opcional)'),
                ),
                const SizedBox(height: 16),
                Row(children: [
                  if (existing != null)
                    TextButton(
                      onPressed: busy
                          ? null
                          : () async {
                              setSheetState(() {
                                busy = true;
                                localError = null;
                              });
                              final ok = await store.deleteTrackedDate(existing.id);
                              if (!ctx.mounted) return;
                              if (ok) {
                                Navigator.pop(ctx);
                              } else {
                                setSheetState(() {
                                  busy = false;
                                  localError = store.error;
                                });
                              }
                            },
                      child: Text('Dispensar', style: TextStyle(color: Theme.of(ctx).colorScheme.error)),
                    ),
                  const Spacer(),
                  TextButton(onPressed: () => Navigator.pop(ctx), child: const Text('Cancelar')),
                  const SizedBox(width: 8),
                  FilledButton(
                    style: FilledButton.styleFrom(backgroundColor: c.accent),
                    onPressed: busy
                        ? null
                        : () async {
                            setSheetState(() {
                              busy = true;
                              localError = null;
                            });
                            final ok = existing == null
                                ? await store.addTrackedDate(
                                    title: titleCtrl.text.trim(),
                                    date: dateCtrl.text.trim(),
                                    project: selectedProject.isEmpty ? null : selectedProject,
                                    note: noteCtrl.text.trim().isEmpty ? null : noteCtrl.text.trim(),
                                  )
                                : await store.editTrackedDate(
                                    existing.id,
                                    title: titleCtrl.text.trim(),
                                    date: dateCtrl.text.trim(),
                                    project: selectedProject,
                                    note: noteCtrl.text.trim(),
                                  );
                            if (!ctx.mounted) return;
                            if (ok) {
                              Navigator.pop(ctx);
                            } else {
                              setSheetState(() {
                                busy = false;
                                localError = store.error;
                              });
                            }
                          },
                    child: const Text('Salvar'),
                  ),
                ]),
              ],
            ),
          ),
        );
      });
    },
  );
}
