// O painel de cota renderiza sem estourar o layout num celular estreito, nos dois temas, e o
// toque no gráfico mostra o valor. As leituras vêm de um store falso (sem o núcleo em Rust).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mobile/models.dart';
import 'package:mobile/store.dart';
import 'package:mobile/theme.dart';
import 'package:mobile/usage.dart';

String _fmt(DateTime t) {
  String two(int n) => n.toString().padLeft(2, '0');
  return '${t.year}-${two(t.month)}-${two(t.day)} ${two(t.hour)}:${two(t.minute)}:${two(t.second)}';
}

UsageSample _sample(String agent, String key, double used, int minutes, DateTime at, DateTime reset) => UsageSample.fromJson({
      'agent': agent,
      'window_key': key,
      'used_percent': used,
      'window_minutes': minutes,
      'resets_at': _fmt(reset),
      'sampled_at': _fmt(at),
    });

class _FakeStore extends ArcaStore {
  final List<UsageSample> samples;
  _FakeStore(this.samples);

  @override
  Future<List<UsageSample>> usageSamples(int days) async => samples;
}

void main() {
  final now = DateTime.now();
  final samples = [
    for (var h = 30; h >= 1; h--)
      _sample('claude-code', 'five_hour', (h % 5) * 18.0, 300, now.subtract(Duration(hours: h)), now.subtract(Duration(hours: h - (h % 5)))),
    _sample('claude-code', 'seven_day', 40, 10080, now.subtract(const Duration(days: 5)), now.add(const Duration(days: 4))),
    _sample('claude-code', 'seven_day', 86, 10080, now.subtract(const Duration(hours: 1)), now.add(const Duration(days: 4))),
    _sample('codex', 'primary', 12, 43200, now.subtract(const Duration(minutes: 10)), now.add(const Duration(days: 20))),
  ];

  for (final brightness in Brightness.values) {
    testWidgets('painel de cota ($brightness)', (tester) async {
      tester.view.physicalSize = const Size(360 * 3, 1600 * 3);
      tester.view.devicePixelRatio = 3;
      addTearDown(tester.view.reset);

      await tester.pumpWidget(MaterialApp(
        theme: arcaTheme(brightness),
        home: Scaffold(body: SingleChildScrollView(padding: const EdgeInsets.all(16), child: UsagePanel(store: _FakeStore(samples)))),
      ));
      await tester.pumpAndSettle();

      expect(find.text('Claude Code · 5 h'), findsOneWidget);
      expect(find.text('Claude Code · 7 dias'), findsOneWidget);
      expect(find.text('Codex · 30 dias'), findsOneWidget);
      expect(find.text('86%'), findsOneWidget);
      expect(find.text('▲ perto do limite'), findsOneWidget);

      // No meio da semana, a leitura de 5 dias atrás (40%) ainda vale; o Codex não tinha leitura.
      for (final (title, expected) in [('Claude Code · 7 dias', ': 40%'), ('Codex · 30 dias', 'sem leitura')]) {
        await tester.tap(find.byKey(ValueKey('usage-chart-$title')));
        await tester.pump();
        expect(find.textContaining(expected), findsOneWidget);
      }

      await tester.tap(find.text('24 h'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    });
  }
}
