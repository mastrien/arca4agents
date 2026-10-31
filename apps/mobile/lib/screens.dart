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

  const TasksScreen({super.key, required this.store, required this.board, required this.list, required this.onList, required this.onAttention});

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
          if (target != null) await store.moveTask(task.id, target);
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
        await store.moveTask(task.id, 'CART');
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
