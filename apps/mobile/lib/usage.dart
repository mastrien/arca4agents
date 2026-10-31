// Cota de uso dos agentes: a mesma seção do desktop (apps/desktop/src/Usage.tsx), em uma coluna.
// As leituras são feitas no PC (statusline do Claude Code, app-server do Codex) e chegam pela
// sincronização. Um card por janela de limite: valor atual, quando zera e a linha em degraus.
// No celular não há hover: tocar ou arrastar no gráfico mostra o valor naquele instante.
import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'models.dart';
import 'screens.dart';
import 'store.dart';
import 'theme.dart';

enum _Range { day, week, month }

const _rangeHours = {_Range.day: 24, _Range.week: 24 * 7, _Range.month: 24 * 30};
const _agentNames = {'claude-code': 'Claude Code', 'codex': 'Codex'};
const _weekdays = ['seg', 'ter', 'qua', 'qui', 'sex', 'sáb', 'dom'];
const _high = 80.0; // a partir daqui o card avisa que está perto do limite

DateTime _parseTime(String s) {
  final parts = s.split(' ');
  final d = parts[0].split('-').map(int.parse).toList();
  final t = (parts.length > 1 ? parts[1] : '00:00:00').split(':').map(int.parse).toList();
  return DateTime(d[0], d[1], d[2], t[0], t[1], t.length > 2 ? t[2] : 0);
}

String _two(int n) => n.toString().padLeft(2, '0');
String _hhmm(DateTime t) => '${_two(t.hour)}:${_two(t.minute)}';
String _ddmm(DateTime t) => '${_two(t.day)}/${_two(t.month)}';
String _dayLabel(DateTime t) => '${_weekdays[t.weekday - 1]} ${_ddmm(t)}';

String _duration(Duration d) {
  final minutes = math.max(0, d.inMinutes);
  final days = minutes ~/ 1440, hours = minutes % 1440 ~/ 60, mins = minutes % 60;
  if (days > 0) return '${days}d ${hours}h';
  if (hours > 0) return '${hours}h ${mins}min';
  return '${mins}min';
}

String _windowLabel(String key, int? minutes) {
  if (minutes == null) return key;
  final base = minutes % 1440 == 0
      ? '${minutes ~/ 1440} dias'
      : minutes % 60 == 0
          ? '${minutes ~/ 60} h'
          : '$minutes min';
  final extra = key.contains(':')
      ? key.split(':').first
      : key.startsWith('seven_day_')
          ? key.substring('seven_day_'.length)
          : null;
  return extra == null ? base : '$base · $extra';
}

class _Point {
  final DateTime t;
  final double v;
  const _Point(this.t, this.v);
}

class _Series {
  final String agent, title;
  final int windowMinutes;
  final UsageSample latest;
  // Degraus: cada ponto vale até o próximo. As viradas conhecidas (resets_at) entram como
  // pontos em 0, porque a cota zera de fato nessa hora.
  final List<_Point> points;
  const _Series(this.agent, this.title, this.windowMinutes, this.latest, this.points);
}

List<_Series> _buildSeries(List<UsageSample> samples, DateTime now) {
  final groups = <String, List<UsageSample>>{};
  for (final s in samples) {
    groups.putIfAbsent('${s.agent}|${s.windowKey}', () => []).add(s);
  }
  final out = <_Series>[];
  for (final list in groups.values) {
    list.sort((a, b) => a.sampledAt.compareTo(b.sampledAt));
    final points = <_Point>[];
    for (var i = 0; i < list.length; i++) {
      final s = list[i];
      final t = _parseTime(s.sampledAt);
      points.add(_Point(t, s.usedPercent));
      final reset = s.resetsAt == null ? null : _parseTime(s.resetsAt!);
      final next = i + 1 < list.length ? _parseTime(list[i + 1].sampledAt) : now;
      if (reset != null && reset.isAfter(t) && reset.isBefore(next)) points.add(_Point(reset, 0));
    }
    final latest = list.last;
    out.add(_Series(
      latest.agent,
      '${_agentNames[latest.agent] ?? latest.agent} · ${_windowLabel(latest.windowKey, latest.windowMinutes)}',
      latest.windowMinutes ?? 1 << 40,
      latest,
      points,
    ));
  }
  out.sort((a, b) {
    final byAgent = a.agent.compareTo(b.agent);
    return byAgent != 0 ? byAgent : a.windowMinutes.compareTo(b.windowMinutes);
  });
  return out;
}

// Valor em `t`: o último ponto em ou antes dele; null antes da primeira leitura.
double? _valueAt(List<_Point> points, DateTime t) {
  double? found;
  for (final p in points) {
    if (p.t.isAfter(t)) break;
    found = p.v;
  }
  return found;
}

/// Painel "Cota dos agentes" da tela de gráficos.
class UsagePanel extends StatefulWidget {
  final ArcaStore store;
  const UsagePanel({super.key, required this.store});

  @override
  State<UsagePanel> createState() => _UsagePanelState();
}

class _UsagePanelState extends State<UsagePanel> {
  late final Future<List<UsageSample>> _load = widget.store.usageSamples(30);
  final DateTime _now = DateTime.now();
  _Range _range = _Range.week;

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    return Panel(
      child: FutureBuilder(
        future: _load,
        builder: (context, snap) {
          final header = [
            const Text('Cota dos agentes', style: TextStyle(fontSize: 15, fontWeight: FontWeight.w600)),
            const SizedBox(height: 2),
            Text('percentual usado de cada janela de limite, lido no PC', style: TextStyle(fontSize: 12, color: c.muted)),
          ];
          if (snap.hasError) {
            return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              ...header,
              const SizedBox(height: 8),
              Text('${snap.error}', style: TextStyle(color: c.attention)),
            ]);
          }
          if (!snap.hasData) {
            return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              ...header,
              const Padding(padding: EdgeInsets.all(16), child: Center(child: CircularProgressIndicator())),
            ]);
          }
          final series = _buildSeries(snap.data!, _now);
          final from = _now.subtract(Duration(hours: _rangeHours[_range]!));
          return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            ...header,
            const SizedBox(height: 10),
            if (series.isEmpty)
              Text('Nenhuma leitura ainda. Elas chegam do PC na sincronização.', style: TextStyle(color: c.muted))
            else ...[
              SegmentedButton<_Range>(
                showSelectedIcon: false,
                style: const ButtonStyle(visualDensity: VisualDensity.compact),
                segments: const [
                  ButtonSegment(value: _Range.day, label: Text('24 h')),
                  ButtonSegment(value: _Range.week, label: Text('7 dias')),
                  ButtonSegment(value: _Range.month, label: Text('30 dias')),
                ],
                selected: {_range},
                onSelectionChanged: (s) => setState(() => _range = s.first),
              ),
              for (final s in series) ...[
                const SizedBox(height: 12),
                _UsageCard(series: s, from: from, now: _now, range: _range),
              ],
            ],
          ]);
        },
      ),
    );
  }
}

class _UsageCard extends StatefulWidget {
  final _Series series;
  final DateTime from, now;
  final _Range range;
  const _UsageCard({required this.series, required this.from, required this.now, required this.range});

  @override
  State<_UsageCard> createState() => _UsageCardState();
}

class _UsageCardState extends State<_UsageCard> {
  double? _touch; // fração 0-1 do eixo x tocada

  @override
  Widget build(BuildContext context) {
    final c = context.arca;
    final s = widget.series;
    final now = widget.now, from = widget.from;
    final reset = s.latest.resetsAt == null ? null : _parseTime(s.latest.resetsAt!);
    final resetPassed = reset != null && !reset.isAfter(now);
    // Depois da virada a cota está em 0 até a próxima leitura.
    final current = resetPassed ? 0.0 : s.latest.usedPercent;
    final high = current >= _high;
    final read = _parseTime(s.latest.sampledAt);
    final span = now.difference(from);
    final touchT = _touch == null ? null : from.add(span * _touch!);
    final touchV = touchT == null ? null : _valueAt(s.points, touchT);
    final mid = from.add(span ~/ 2);
    final axis = TextStyle(fontSize: 10, color: c.muted, fontFamily: mono);

    return Container(
      padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
      decoration: BoxDecoration(
        border: Border.all(color: c.line),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Row(children: [
          Expanded(child: Text(s.title, style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: c.muted))),
          // Estado reservado: cor de atenção sempre com ícone e texto.
          if (high) Text('▲ perto do limite', style: TextStyle(fontSize: 11, fontWeight: FontWeight.w600, color: c.attention)),
        ]),
        Row(crossAxisAlignment: CrossAxisAlignment.baseline, textBaseline: TextBaseline.alphabetic, children: [
          Text('${current.round()}%', style: const TextStyle(fontSize: 26, fontWeight: FontWeight.w600, height: 1.2, fontFeatures: [FontFeature.tabularFigures()])),
          const SizedBox(width: 10),
          Flexible(
            child: Text(
              reset == null ? '' : (resetPassed ? 'zerou às ${_hhmm(reset)}' : 'zera em ${_duration(reset.difference(now))}'),
              style: TextStyle(fontSize: 12, color: c.muted),
            ),
          ),
        ]),
        const SizedBox(height: 6),
        ClipRRect(
          borderRadius: BorderRadius.circular(3),
          child: Container(
            height: 6,
            color: c.lineSoft,
            alignment: Alignment.centerLeft,
            child: FractionallySizedBox(widthFactor: (current / 100).clamp(0, 1), child: Container(color: high ? c.attentionDot : c.accent)),
          ),
        ),
        const SizedBox(height: 10),
        // Valor do ponto tocado, com altura fixa para o gráfico não pular.
        SizedBox(
          height: 18,
          child: Text(
            touchT == null ? 'Toque no gráfico para ver o valor.' : '${_dayLabel(touchT)} ${_hhmm(touchT)}: ${touchV == null ? 'sem leitura' : '${touchV.round()}%'}',
            style: TextStyle(fontSize: 12, color: touchT == null ? c.muted : c.text),
          ),
        ),
        Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          SizedBox(
            width: 24,
            height: 90,
            child: Stack(clipBehavior: Clip.none, children: [
              for (final t in const [0, 50, 100]) Positioned(right: 0, bottom: t / 100 * 90 - 6, child: Text('$t', style: axis)),
            ]),
          ),
          const SizedBox(width: 6),
          Expanded(
            child: Column(children: [
              LayoutBuilder(builder: (context, box) {
                void touch(Offset p) => setState(() => _touch = (p.dx / box.maxWidth).clamp(0.0, 1.0));
                return GestureDetector(
                  key: ValueKey('usage-chart-${s.title}'),
                  behavior: HitTestBehavior.opaque,
                  onTapDown: (d) => touch(d.localPosition),
                  onHorizontalDragUpdate: (d) => touch(d.localPosition),
                  child: SizedBox(
                    height: 90,
                    width: double.infinity,
                    child: CustomPaint(
                      painter: _StepPainter(
                        points: s.points,
                        from: from,
                        now: now,
                        touch: _touch,
                        line: c.accent,
                        grid: c.line,
                        base: c.muted,
                        ring: c.surface,
                      ),
                    ),
                  ),
                );
              }),
              const SizedBox(height: 4),
              Row(children: [
                Text(widget.range == _Range.day ? _hhmm(from) : _ddmm(from), style: axis),
                const Spacer(),
                Text(widget.range == _Range.day ? _hhmm(mid) : _ddmm(mid), style: axis),
                const Spacer(),
                Text('agora', style: axis),
              ]),
            ]),
          ),
        ]),
        const SizedBox(height: 6),
        Text('lido há ${_duration(now.difference(read))}', style: TextStyle(fontSize: 12, color: c.muted)),
      ]),
    );
  }
}

class _StepPainter extends CustomPainter {
  final List<_Point> points;
  final DateTime from, now;
  final double? touch;
  final Color line, grid, base, ring;

  _StepPainter({required this.points, required this.from, required this.now, required this.touch, required this.line, required this.grid, required this.base, required this.ring});

  @override
  void paint(Canvas canvas, Size size) {
    final span = now.difference(from).inMilliseconds.toDouble();
    double x(DateTime t) => t.difference(from).inMilliseconds / span * size.width;
    double y(double v) => size.height - (v.clamp(0, 100) / 100) * size.height;

    final gridPaint = Paint()
      ..color = grid
      ..strokeWidth = 1;
    for (final v in [50.0, _high]) {
      // Tracejado recessivo.
      for (var dx = 0.0; dx < size.width; dx += 6) {
        canvas.drawLine(Offset(dx, y(v)), Offset(math.min(dx + 3, size.width), y(v)), gridPaint);
      }
    }
    canvas.drawLine(Offset(0, size.height), Offset(size.width, size.height), Paint()
      ..color = base.withValues(alpha: 0.5)
      ..strokeWidth = 1);

    // Começa no valor de antes do período (se houver) e vai em degraus até agora.
    final start = _valueAt(points, from);
    final steps = <_Point>[
      if (start != null) _Point(from, start),
      ...points.where((p) => p.t.isAfter(from) && !p.t.isAfter(now)),
    ];
    if (steps.isNotEmpty) {
      final path = Path()..moveTo(x(steps.first.t), y(steps.first.v));
      var lastY = y(steps.first.v);
      for (final p in steps.skip(1)) {
        path
          ..lineTo(x(p.t), lastY)
          ..lineTo(x(p.t), y(p.v));
        lastY = y(p.v);
      }
      path.lineTo(size.width, y(steps.last.v));
      final area = Path.from(path)
        ..lineTo(size.width, size.height)
        ..lineTo(x(steps.first.t), size.height)
        ..close();
      canvas.drawPath(area, Paint()..color = line.withValues(alpha: 0.12));
      canvas.drawPath(path, Paint()
        ..color = line
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2
        ..strokeJoin = StrokeJoin.round);
    }

    if (touch != null) {
      final tx = touch! * size.width;
      canvas.drawLine(Offset(tx, 0), Offset(tx, size.height), Paint()
        ..color = base
        ..strokeWidth = 1);
      final v = _valueAt(points, from.add(Duration(milliseconds: (span * touch!).round())));
      if (v != null) {
        canvas.drawCircle(Offset(tx, y(v)), 6, Paint()..color = ring);
        canvas.drawCircle(Offset(tx, y(v)), 4, Paint()..color = line);
      }
    }
  }

  @override
  bool shouldRepaint(_StepPainter old) => old.points != points || old.from != from || old.touch != touch || old.line != line;
}
