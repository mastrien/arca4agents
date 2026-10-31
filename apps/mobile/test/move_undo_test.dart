// Mover uma tarefa mostra o aviso com o que mudou, e "Desfazer" devolve ao status anterior.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mobile/models.dart';
import 'package:mobile/screens.dart';
import 'package:mobile/store.dart';
import 'package:mobile/theme.dart';

class _FakeStore extends ArcaStore {
  final moves = <(int, String)>[];

  @override
  Future<bool> moveTask(int id, String status) async {
    moves.add((id, status));
    return true;
  }
}

TaskView _task(String status) => TaskView.fromJson({
      'id': 7,
      'title': 'Revisar o PR',
      'status': status,
      'daily': false,
      'due_date': null,
      'due_label': null,
      'due_state': null,
      'project': null,
      'agent': null,
      'working_agents': <String>[],
      'done_time': null,
    });

void main() {
  for (final (from, to, message) in [
    ('SHELF', 'CART', 'foi para o carrinho'),
    ('CART', 'DONE', 'concluída'),
    ('CART', 'SHELF', 'foi para a estante'),
    ('DONE', 'CART', 'reaberta'),
  ]) {
    testWidgets('$from → $to', (tester) async {
      final store = _FakeStore();
      await tester.pumpWidget(MaterialApp(
        theme: arcaTheme(Brightness.light),
        home: Scaffold(
          body: Builder(
            builder: (context) => TextButton(onPressed: () => moveWithUndo(context, store, _task(from), to), child: const Text('mover')),
          ),
        ),
      ));

      await tester.tap(find.text('mover'));
      await tester.pumpAndSettle();
      expect(find.text('“Revisar o PR” $message'), findsOneWidget);

      await tester.tap(find.text('Desfazer'));
      await tester.pumpAndSettle();
      expect(store.moves, [(7, to), (7, from)]);
    });
  }

  testWidgets('o aviso some sozinho', (tester) async {
    final store = _FakeStore();
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: Builder(builder: (context) => TextButton(onPressed: () => moveWithUndo(context, store, _task('SHELF'), 'CART'), child: const Text('mover'))),
      ),
    ));
    await tester.tap(find.text('mover'));
    await tester.pumpAndSettle();
    expect(find.text('Desfazer'), findsOneWidget);
    await tester.pump(const Duration(seconds: 5));
    await tester.pumpAndSettle();
    expect(find.text('Desfazer'), findsNothing);
  });
}
