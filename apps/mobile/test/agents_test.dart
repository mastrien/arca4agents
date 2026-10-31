// Tela de agentes: sessão sem projeto mostra a origem, e sessão com Remote Control ganha o botão
// "Abrir no Claude" (no lugar do "responda no PC").
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mobile/models.dart';
import 'package:mobile/screens.dart';
import 'package:mobile/store.dart';
import 'package:mobile/theme.dart';

Map<String, dynamic> _session(String id, String activity, {String? project, String? origin, String? url}) => {
      'session_id': id,
      'agent': 'claude-code',
      'project': project,
      'cwd': 'c:/x',
      'activity': activity,
      'activity_label': activity == 'waiting' ? 'esperando você' : 'trabalhando',
      'last_seen_label': 'agora há pouco',
      'task_id': null,
      'task_title': null,
      'origin': origin,
      'remote_url': url,
    };

void main() {
  testWidgets('origem e link do Remote Control', (tester) async {
    final board = Board.fromJson({
      'today': '2026-10-31',
      'shelf': [],
      'cart': [],
      'done': [],
      'suggestions': [],
      'projects': [],
      'window_minutes': 10,
      'sessions': [
        _session('a', 'waiting', project: 'arca4agents', url: 'https://claude.ai/code/session_01X'),
        _session('b', 'working', origin: 'app Claude · sessão lateral'),
        _session('c', 'waiting', origin: '~/desktop/notas'),
      ],
    });

    await tester.pumpWidget(MaterialApp(
      theme: arcaTheme(Brightness.light),
      home: Scaffold(body: AgentsScreen(store: ArcaStore(), board: board)),
    ));

    // Com link: botão no cartão de espera e na lista; sem link: "responda no PC".
    expect(find.text('Abrir no Claude'), findsNWidgets(2));
    expect(find.textContaining('responda no PC'), findsOneWidget);
    expect(find.textContaining('~/desktop/notas'), findsWidgets);
    expect(find.textContaining('app Claude · sessão lateral'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
