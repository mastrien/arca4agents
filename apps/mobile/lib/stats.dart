// Tela de gráficos: a mesma do app desktop (apps/desktop/src/Stats.tsx), em uma coluna.
// Tudo é calculado aqui a partir do histórico, das fotos diárias de estante/carrinho e do board
// sem filtro. No celular não há hover: tocar numa coluna mostra os números dela.
import 'package:flutter/material.dart';

import 'models.dart';
import 'screens.dart';
import 'store.dart';
import 'theme.dart';
import 'usage.dart';

void openStats(BuildContext context, ArcaStore store) {
  Navigator.push(context, MaterialPageRoute(builder: (_) => StatsScreen(store: store)));
}

// ---------- Dados ----------

class _DoneEntry {
  final String day; // AAAA-MM-DD
  final String? project, agent;
  final bool daily;
  const _DoneEntry(this.day, this.project, this.agent, this.daily);
}

enum _Series { shelf, cart, done }

const _seriesLabel = {_Series.shelf: 'Estante', _Series.cart: 'Carrinho', _Series.done: 'Concluídas'};

// Slots 1-3 da paleta categórica validada (todos os pares passam no teste de daltonismo nos dois
// modos); o modo escuro tem degraus próprios. As mesmas do desktop.
Color _seriesColor(BuildContext context, _Series s) {
  final dark = Theme.of(context).brightness == Brightness.dark;
  return switch (s) {
    _Series.shelf => dark ? const Color(0xFF3987E5) : const Color(0xFF2A78D6),
    _Series.cart => dark ? const Color(0xFFD95926) : const Color(0xFFEB6834),
    _Series.done => dark ? const Color(0xFF199E70) : const Color(0xFF1BAF7A),
  };
}

class _TimeBucket {
  final String label, tip;
  final int done; // concluídas durante o período
  final int? shelf, cart; // tamanho no fim do período; null antes da primeira foto
  const _TimeBucket(this.label, this.tip, this.done, this.shelf, this.cart);

  int value(_Series s) => switch (s) { _Series.shelf => shelf ?? 0, _Series.cart => cart ?? 0, _Series.done => done };
  bool missing(_Series s) => s != _Series.done && (s == _Series.shelf ? shelf : cart) == null;
}

class _Ranked {
  final String name;
  final int count;
  final bool isNone;
  const _Ranked(this.name, this.count, this.isNone);
}

enum _Granularity { day, week, month }

const _months = ['jan', 'fev', 'mar', 'abr', 'mai', 'jun', 'jul', 'ago', 'set', 'out', 'nov', 'dez'];
const _weekdays = ['seg', 'ter', 'qua', 'qui', 'sex', 'sáb', 'dom'];

DateTime _parseDay(String s) {
  final p = s.substring(0, 10).split('-').map(int.parse).toList();
  return DateTime(p[0], p[1], p[2]);
}

String _two(int n) => n.toString().padLeft(2, '0');
String _dayKey(DateTime d) => '${d.year}-${_two(d.month)}-${_two(d.day)}';
DateTime _addDays(DateTime d, int n) => DateTime(d.year, d.month, d.day + n);
DateTime _startOfWeek(DateTime d) => _addDays(d, 1 - d.weekday); // segunda (weekday 1) a domingo (7)
String _ddmm(DateTime d) => '${_two(d.day)}/${_two(d.month)}';
String _weekdayOf(DateTime d) => _weekdays[d.weekday - 1];

// Estado da lista no fim do dia `key`: a foto desse dia ou a última antes dele, porque um dia sem
// foto é um dia sem escritas nas tarefas.
StatusSnapshot? _snapshotAt(List<StatusSnapshot> snapshots, String key) {
  StatusSnapshot? found;
  for (final s in snapshots) {
    if (s.date.compareTo(key) > 0) break;
    found = s;
  }
  return found;
}

List<_TimeBucket> _timeline(List<_DoneEntry> entries, List<StatusSnapshot> snapshots, DateTime today, _Granularity g) {
  String keyOf(DateTime d) => switch (g) {
        _Granularity.day => _dayKey(d),
        _Granularity.week => _dayKey(_startOfWeek(d)),
        _Granularity.month => '${d.year}-${d.month}',
      };
  final counts = <String, int>{};
  for (final e in entries) {
    final k = keyOf(_parseDay(e.day));
    counts[k] = (counts[k] ?? 0) + 1;
  }

  final out = <_TimeBucket>[];
  // `start` identifica o período; `end` é o dia cuja foto vale para ele (no máximo hoje).
  void push(String label, String tip, DateTime start, DateTime end) {
    final snap = _snapshotAt(snapshots, _dayKey(end.isAfter(today) ? today : end));
    out.add(_TimeBucket(label, tip, counts[keyOf(start)] ?? 0, snap?.shelf, snap?.cart));
  }

  switch (g) {
    case _Granularity.day:
      for (var i = 29; i >= 0; i--) {
        final d = _addDays(today, -i);
        push(_ddmm(d), '${_weekdayOf(d)}, ${_ddmm(d)}', d, d);
      }
    case _Granularity.week:
      final current = _startOfWeek(today);
      for (var i = 11; i >= 0; i--) {
        final d = _addDays(current, -7 * i);
        push(_ddmm(d), 'semana de ${_ddmm(d)} a ${_ddmm(_addDays(d, 6))}', d, _addDays(d, 6));
      }
    case _Granularity.month:
      for (var i = 11; i >= 0; i--) {
        final d = DateTime(today.year, today.month - i, 1);
        final m = _months[d.month - 1];
        final label = d.month == 1 || i == 11 ? '$m ${d.year.toString().substring(2)}' : m;
        push(label, '$m de ${d.year}', d, DateTime(d.year, d.month + 1, 0));
      }
  }
  return out;
}

List<_Ranked> _groupCount(Iterable<String?> keys, {String none = 'sem projeto'}) {
  final map = <String, int>{};
  for (final k in keys) {
    map[k ?? ''] = (map[k ?? ''] ?? 0) + 1;
  }
  final rows = map.entries.map((e) => _Ranked(e.key.isEmpty ? none : e.key, e.value, e.key.isEmpty)).toList()
    ..sort((a, b) => b.count != a.count ? b.count - a.count : a.name.compareTo(b.name));
  return rows;
}

// Escala do eixo: passos de 1, 2 ou 5 × 10^k, uns 4 intervalos.
({int top, List<int> ticks}) _niceScale(int max) {
  if (max <= 0) return (top: 4, ticks: [0, 2, 4]);
  final raw = max / 4;
  var pow = 1;
  while (pow * 10 <= raw) {
    pow *= 10;
  }
  final step = [1, 2, 5, 10].map((m) => m * pow).firstWhere((s) => s >= raw, orElse: () => pow * 10);
  final top = (max / step).ceil() * step;
  return (top: top, ticks: [for (var t = 0; t <= top; t += step) t]);
}

String _percent(int part, int total) {
  if (total == 0) return '0%';
  final p = part / total * 100;
  return p > 0 && p < 1 ? '<1%' : '${p.round()}%';
}

class _StatsData {
  final int total, thisWeek, lastWeek, streak, byAgent, cartTotal, shelfTotal;
  final double weeklyAverage;
  final List<_TimeBucket> timeline;
  final List<int> byWeekday;
  final List<_Ranked> doneByProject, doneByAuthor, cartByProject, shelfByProject;
  final String? trackedSince;

  _StatsData({
    required this.total,
    required this.thisWeek,
    required this.lastWeek,
    required this.streak,
    required this.byAgent,
    required this.cartTotal,
    required this.shelfTotal,
    required this.weeklyAverage,
    required this.timeline,
    required this.byWeekday,
    required this.doneByProject,
    required this.doneByAuthor,
    required this.cartByProject,
    required this.shelfByProject,
    required this.trackedSince,
  });

  factory _StatsData.compute(List<HistoryView> history, List<StatusSnapshot> snapshots, Board board, _Granularity g, bool includeDaily) {
    final today = _parseDay(board.today);
    // O histórico só recebe as concluídas na virada do dia; as de hoje vêm do board.
    final all = [
      for (final h in history) _DoneEntry(h.date.substring(0, 10), h.project, h.agent, h.daily),
      for (final t in board.done) _DoneEntry(board.today, t.project, t.agent, t.daily),
    ];
    final done = includeDaily ? all : all.where((e) => !e.daily).toList();

    final thisWeek = _dayKey(_startOfWeek(today));
    final lastWeek = _dayKey(_addDays(_startOfWeek(today), -7));
    final eightWeeksAgo = _dayKey(_addDays(_startOfWeek(today), -56));
    String weekKey(_DoneEntry e) => _dayKey(_startOfWeek(_parseDay(e.day)));

    final days = done.map((e) => e.day).toSet();
    var streak = 0;
    var cursor = days.contains(board.today) ? today : _addDays(today, -1); // hoje ainda pode entrar
    while (days.contains(_dayKey(cursor))) {
      streak++;
      cursor = _addDays(cursor, -1);
    }

    final byWeekday = List.filled(7, 0);
    for (final e in done) {
      byWeekday[_parseDay(e.day).weekday - 1]++;
    }

    return _StatsData(
      total: done.length,
      thisWeek: done.where((e) => weekKey(e) == thisWeek).length,
      lastWeek: done.where((e) => weekKey(e) == lastWeek).length,
      weeklyAverage: done.where((e) => e.day.compareTo(eightWeeksAgo) >= 0 && weekKey(e) != thisWeek).length / 8,
      streak: streak,
      byAgent: done.where((e) => e.agent != null).length,
      timeline: _timeline(done, snapshots, today, g),
      trackedSince: snapshots.isEmpty ? null : snapshots.first.date,
      byWeekday: byWeekday,
      doneByProject: _groupCount(done.map((e) => e.project)),
      doneByAuthor: _groupCount(done.map((e) => e.agent ?? 'você')),
      cartByProject: _groupCount(board.cart.map((t) => t.project)),
      shelfByProject: _groupCount(board.shelf.map((t) => t.project)),
      cartTotal: board.cart.length,
      shelfTotal: board.shelf.length,
    );
  }
}

// ---------- Tela ----------

class StatsScreen extends StatefulWidget {
  final ArcaStore store;
  const StatsScreen({super.key, required this.store});

  @override
  State<StatsScreen> createState() => _StatsScreenState();
}

class _StatsScreenState extends State<StatsScreen> {
  late final Future<(List<HistoryView>, List<StatusSnapshot>, Board)> _load = () async {
    final s = widget.store;
    return (await s.history(), await s.statusSnapshots(), await s.boardAll());
  }();

  _Granularity _granularity = _Granularity.week;
  bool _includeDaily = true;
  final Map<_Series, bool> _visible = {for (final s in _Series.values) s: true};
  int? _selectedBucket;
  int? _selectedWeekday;

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Scaffold(
      body: SafeArea(
        child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          ScreenHeader(
            'Gráficos',
            back: true,
            trailing: FilterChip(
              label: const Text('contar diárias'),
              selected: _includeDaily,
              showCheckmark: true,
              visualDensity: VisualDensity.compact,
              onSelected: (v) => setState(() => _includeDaily = v),
            ),
          ),
          Expanded(
            child: FutureBuilder(
              future: _load,
              builder: (context, snap) {
                if (snap.hasError) return Center(child: Text('${snap.error}', style: TextStyle(color: c.attention)));
                if (!snap.hasData) return const Center(child: CircularProgressIndicator());
                final (history, snapshots, board) = snap.data!;
                final data = _StatsData.compute(history, snapshots, board, _granularity, _includeDaily);
                return ListView(padding: const EdgeInsets.fromLTRB(16, 4, 16, 28), children: [
                  _tiles(data),
                  const SizedBox(height: 14),
                  UsagePanel(store: widget.store),
                  const SizedBox(height: 14),
                  _timelinePanel(context, data),
                  const SizedBox(height: 12),
                  _weekdayPanel(context, data),
                  const SizedBox(height: 12),
                  _rankedPanel('Carrinho por projeto', '${data.cartTotal}', data.cartByProject, data.cartTotal, 'Carrinho vazio.'),
                  const SizedBox(height: 12),
                  _rankedPanel('Estante por projeto', '${data.shelfTotal}', data.shelfByProject, data.shelfTotal, 'Estante vazia.'),
                  const SizedBox(height: 12),
                  _rankedPanel('Concluídas por projeto', 'desde o início', data.doneByProject, data.total, 'Nada concluído ainda.'),
                  const SizedBox(height: 12),
                  _rankedPanel('Concluídas por autor da tarefa', 'quem criou', data.doneByAuthor, data.total, 'Nada concluído ainda.', neutral: true),
                ]);
              },
            ),
          ),
        ]),
      ),
    );
  }

  Widget _tiles(_StatsData d) {
    final avg = d.weeklyAverage.toStringAsFixed(1).replaceAll('.', ',').replaceFirst(RegExp(r',0$'), '');
    final tiles = [
      _Tile('Esta semana', '${d.thisWeek}', 'semana passada: ${d.lastWeek}'),
      _Tile('Média por semana', avg, 'últimas 8 semanas completas'),
      _Tile('Sequência', '${d.streak}', d.streak == 1 ? 'dia seguido concluindo algo' : 'dias seguidos concluindo algo'),
      _Tile('Concluídas no total', '${d.total}', '${_percent(d.byAgent, d.total)} criadas por agentes'),
      _Tile('Abertas', '${d.cartTotal + d.shelfTotal}', '${d.cartTotal} no carrinho · ${d.shelfTotal} na estante'),
    ];
    // Dois por linha, com a mesma altura na linha; o último fica com a largura toda se sobrar sozinho.
    return Column(children: [
      for (var i = 0; i < tiles.length; i += 2) ...[
        if (i > 0) const SizedBox(height: 8),
        IntrinsicHeight(
          child: Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            Expanded(child: tiles[i]),
            if (i + 1 < tiles.length) ...[const SizedBox(width: 8), Expanded(child: tiles[i + 1])],
          ]),
        ),
      ],
    ]);
  }

  Widget _timelinePanel(BuildContext context, _StatsData d) {
    final c = context.arca;
    final shown = _Series.values.where((s) => _visible[s]!).toList();
    final selected = _selectedBucket != null && _selectedBucket! < d.timeline.length ? d.timeline[_selectedBucket!] : null;
    return Panel(
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        const Text('Ao longo do tempo', style: TextStyle(fontSize: 15, fontWeight: FontWeight.w600)),
        const SizedBox(height: 10),
        SegmentedButton<_Granularity>(
          showSelectedIcon: false,
          style: const ButtonStyle(visualDensity: VisualDensity.compact),
          segments: const [
            ButtonSegment(value: _Granularity.day, label: Text('Dia')),
            ButtonSegment(value: _Granularity.week, label: Text('Semana')),
            ButtonSegment(value: _Granularity.month, label: Text('Mês')),
          ],
          selected: {_granularity},
          onSelectionChanged: (s) => setState(() {
            _granularity = s.first;
            _selectedBucket = null;
          }),
        ),
        const SizedBox(height: 8),
        Wrap(spacing: 6, runSpacing: 6, children: [
          for (final s in _Series.values)
            _SeriesToggle(
              label: _seriesLabel[s]!,
              color: _seriesColor(context, s),
              on: _visible[s]!,
              onTap: () => setState(() => _visible[s] = !_visible[s]!),
            ),
        ]),
        const SizedBox(height: 10),
        // Detalhe da coluna tocada, com altura fixa para o gráfico não pular.
        SizedBox(
          height: 36,
          child: selected == null
              ? Align(alignment: Alignment.centerLeft, child: Text('Toque numa coluna para ver os números.', style: TextStyle(fontSize: 12, color: c.muted)))
              : Column(crossAxisAlignment: CrossAxisAlignment.start, mainAxisAlignment: MainAxisAlignment.center, children: [
                  Text(selected.tip, style: TextStyle(fontSize: 12, color: c.muted)),
                  Wrap(spacing: 12, children: [
                    for (final s in shown.reversed)
                      Row(mainAxisSize: MainAxisSize.min, children: [
                        _Swatch(_seriesColor(context, s)),
                        const SizedBox(width: 5),
                        Text('${_seriesLabel[s]} ', style: const TextStyle(fontSize: 13)),
                        Text(selected.missing(s) ? '—' : '${selected.value(s)}', style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600)),
                      ]),
                  ]),
                ]),
        ),
        const SizedBox(height: 6),
        _BarChart(
          stacks: [
            for (final b in d.timeline) [for (final s in shown) (_seriesColor(context, s), b.value(s))],
          ],
          labels: d.timeline.map((b) => b.label).toList(),
          labelEvery: switch (_granularity) { _Granularity.day => 6, _Granularity.week => 3, _Granularity.month => 2 },
          selected: _selectedBucket,
          onTap: (i) => setState(() => _selectedBucket = _selectedBucket == i ? null : i),
        ),
        if (d.trackedSince != null && (_visible[_Series.shelf]! || _visible[_Series.cart]!)) ...[
          const SizedBox(height: 10),
          Text(
            'Estante e carrinho: tamanho da lista no fim de cada período, registrado desde '
            '${d.trackedSince!.split('-').reversed.join('/')}. Concluídas: histórico inteiro.',
            style: TextStyle(fontSize: 12, color: c.muted, height: 1.35),
          ),
        ],
      ]),
    );
  }

  Widget _weekdayPanel(BuildContext context, _StatsData d) {
    final c = context.arca;
    final i = _selectedWeekday;
    return Panel(
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Row(children: [
          const Expanded(child: Text('Por dia da semana', style: TextStyle(fontSize: 15, fontWeight: FontWeight.w600))),
          Text(i == null ? 'desde o início' : '${_weekdays[i]}: ${d.byWeekday[i]} ${d.byWeekday[i] == 1 ? 'concluída' : 'concluídas'}',
              style: TextStyle(fontSize: 12, color: i == null ? c.muted : c.text)),
        ]),
        const SizedBox(height: 12),
        _BarChart(
          stacks: [for (final n in d.byWeekday) [(c.accent, n)]],
          labels: _weekdays,
          labelEvery: 1,
          height: 130,
          selected: i,
          onTap: (n) => setState(() => _selectedWeekday = _selectedWeekday == n ? null : n),
        ),
      ]),
    );
  }

  Widget _rankedPanel(String title, String hint, List<_Ranked> rows, int total, String empty, {bool neutral = false}) {
    final c = context.arca;
    final max = rows.isEmpty ? 1 : rows.map((r) => r.count).reduce((a, b) => a > b ? a : b);
    return Panel(
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Row(children: [
          Expanded(child: Text(title, style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600))),
          Text(hint, style: TextStyle(fontSize: 12, color: c.muted, fontFamily: hint.startsWith(RegExp(r'\d')) ? mono : null)),
        ]),
        const SizedBox(height: 10),
        if (rows.isEmpty) Text(empty, style: TextStyle(color: c.muted)),
        for (final r in rows)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 4),
            child: _RankedRow(row: r, max: max, total: total, color: neutral ? c.accent : (r.isNone ? c.idle : projectColor(r.name)), showDot: !neutral),
          ),
      ]),
    );
  }
}

// ---------- Peças ----------

class _Tile extends StatelessWidget {
  final String label, value, note;
  const _Tile(this.label, this.value, this.note);

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Panel(
      padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Text(label, style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: c.muted)),
        Text(value, style: const TextStyle(fontSize: 26, fontWeight: FontWeight.w600, height: 1.2, fontFeatures: [FontFeature.tabularFigures()])),
        Text(note, style: TextStyle(fontSize: 12, color: c.muted), maxLines: 2),
      ]),
    );
  }
}

class _Swatch extends StatelessWidget {
  final Color? color;
  final Color? outline;
  const _Swatch(this.color, {this.outline});

  @override
  Widget build(BuildContext context) => Container(
        width: 10,
        height: 10,
        decoration: BoxDecoration(
          color: color,
          borderRadius: BorderRadius.circular(3),
          border: outline != null ? Border.all(color: outline!, width: 1.5) : null,
        ),
      );
}

/// Liga/desliga uma categoria. Desligada, a amostra vira só contorno (a largura não muda).
class _SeriesToggle extends StatelessWidget {
  final String label;
  final Color color;
  final bool on;
  final VoidCallback onTap;
  const _SeriesToggle({required this.label, required this.color, required this.on, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Semantics(
      button: true,
      toggled: on,
      child: Material(
        color: c.surface,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8), side: BorderSide(color: c.line)),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
            child: Row(mainAxisSize: MainAxisSize.min, children: [
              on ? _Swatch(color) : _Swatch(null, outline: c.idle),
              const SizedBox(width: 7),
              Text(label, style: TextStyle(fontSize: 13, color: on ? c.text : c.muted)),
            ]),
          ),
        ),
      ),
    );
  }
}

/// Barras verticais; cada coluna é uma pilha de (cor, valor), de baixo para cima.
class _BarChart extends StatelessWidget {
  final List<List<(Color, int)>> stacks;
  final List<String> labels;
  final int labelEvery;
  final double height;
  final int? selected;
  final ValueChanged<int> onTap;

  const _BarChart({required this.stacks, required this.labels, required this.labelEvery, required this.onTap, this.selected, this.height = 180});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final totals = [for (final s in stacks) s.fold<int>(0, (sum, p) => sum + p.$2)];
    final scale = _niceScale(totals.fold(0, (a, b) => a > b ? a : b));
    final n = stacks.length;
    // Rótulos contados a partir do fim: a coluna mais recente sempre tem o seu.
    bool showLabel(int i) => (n - 1 - i) % labelEvery == 0;
    final axis = TextStyle(fontSize: 10, color: c.muted, fontFamily: mono);

    return Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
      SizedBox(
        width: 26,
        height: height,
        child: Stack(clipBehavior: Clip.none, children: [
          for (final t in scale.ticks)
            Positioned(right: 0, bottom: t / scale.top * height - 6, child: Text('$t', style: axis)),
        ]),
      ),
      const SizedBox(width: 6),
      Expanded(
        child: Column(children: [
          SizedBox(
            height: height,
            child: Stack(children: [
              for (final t in scale.ticks)
                Positioned(left: 0, right: 0, bottom: t / scale.top * height, child: Container(height: 1, color: t == 0 ? c.line : c.lineSoft)),
              Row(crossAxisAlignment: CrossAxisAlignment.end, children: [
                for (var i = 0; i < n; i++)
                  Expanded(
                    child: GestureDetector(
                      behavior: HitTestBehavior.opaque,
                      onTap: () => onTap(i),
                      child: Container(
                        height: height,
                        margin: const EdgeInsets.symmetric(horizontal: 1),
                        decoration: BoxDecoration(
                          color: selected == i ? c.accentTint : null,
                          borderRadius: const BorderRadius.vertical(top: Radius.circular(4)),
                        ),
                        alignment: Alignment.bottomCenter,
                        child: ConstrainedBox(
                          constraints: const BoxConstraints(maxWidth: 30),
                          child: Column(mainAxisSize: MainAxisSize.min, children: [
                            for (final (j, (color, v)) in stacks[i].where((p) => p.$2 > 0).toList().reversed.indexed)
                              Container(
                                height: v / scale.top * height,
                                // Respiro de 1,5 px da superfície entre as fatias.
                                margin: EdgeInsets.only(top: j == 0 ? 0 : 1.5),
                                decoration: BoxDecoration(
                                  color: color,
                                  borderRadius: j == 0 ? const BorderRadius.vertical(top: Radius.circular(3)) : null,
                                ),
                              ),
                          ]),
                        ),
                      ),
                    ),
                  ),
              ]),
            ]),
          ),
          const SizedBox(height: 4),
          Row(children: [
            for (var i = 0; i < n; i++)
              Expanded(
                child: Text(showLabel(i) ? labels[i] : '', textAlign: TextAlign.center, maxLines: 1, softWrap: false, overflow: TextOverflow.visible, style: axis),
              ),
          ]),
        ]),
      ),
    ]);
  }
}

class _RankedRow extends StatelessWidget {
  final _Ranked row;
  final int max, total;
  final Color color;
  final bool showDot;
  const _RankedRow({required this.row, required this.max, required this.total, required this.color, required this.showDot});

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Row(children: [
      SizedBox(
        width: 112,
        child: Row(children: [
          if (showDot) ...[
            Container(width: 8, height: 8, decoration: BoxDecoration(color: color, borderRadius: BorderRadius.circular(2))),
            const SizedBox(width: 6),
          ],
          Flexible(
            child: Text(row.name, overflow: TextOverflow.ellipsis, style: TextStyle(fontSize: 13, color: row.isNone ? c.muted : c.text)),
          ),
        ]),
      ),
      const SizedBox(width: 8),
      Expanded(
        child: ClipRRect(
          borderRadius: BorderRadius.circular(99),
          child: Container(
            height: 10,
            color: c.lineSoft,
            alignment: Alignment.centerLeft,
            child: FractionallySizedBox(widthFactor: row.count / max, child: Container(color: color)),
          ),
        ),
      ),
      const SizedBox(width: 8),
      SizedBox(
        width: 74,
        child: Text.rich(
          TextSpan(children: [
            TextSpan(text: _percent(row.count, total)),
            TextSpan(text: ' · ${row.count}', style: TextStyle(color: c.muted)),
          ]),
          textAlign: TextAlign.right,
          style: const TextStyle(fontSize: 12, fontFamily: mono),
        ),
      ),
    ]);
  }
}
