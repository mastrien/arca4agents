// Modelos do JSON devolvido por `arca_core::api` (ver rust/src/api/arca.rs).

class TaskView {
  final int id;
  final String title;
  final String status; // SHELF, CART, DONE
  final bool daily;
  final String? dueDate;
  final String? dueLabel;
  final String? dueState; // overdue, today, upcoming
  final String? project;
  final String? agent;
  final List<String> workingAgents;
  final String? doneTime;

  TaskView.fromJson(Map<String, dynamic> j)
      : id = j['id'],
        title = j['title'],
        status = j['status'],
        daily = j['daily'],
        dueDate = j['due_date'],
        dueLabel = j['due_label'],
        dueState = j['due_state'],
        project = j['project'],
        agent = j['agent'],
        workingAgents = List<String>.from(j['working_agents']),
        doneTime = j['done_time'];
}

class SessionView {
  final String sessionId;
  final String agent;
  final String? project;
  final String activity; // working, possibly_stopped, waiting, ended
  final String activityLabel;
  final String lastSeenLabel;
  final int? taskId;
  final String? taskTitle;

  SessionView.fromJson(Map<String, dynamic> j)
      : sessionId = j['session_id'],
        agent = j['agent'],
        project = j['project'],
        activity = j['activity'],
        activityLabel = j['activity_label'],
        lastSeenLabel = j['last_seen_label'],
        taskId = j['task_id'],
        taskTitle = j['task_title'];
}

class SuggestionView {
  final int id;
  final int taskId;
  final String taskTitle;
  final String agent;
  final String summary;
  final String? reason;
  final String createdLabel;

  SuggestionView.fromJson(Map<String, dynamic> j)
      : id = j['id'],
        taskId = j['task_id'],
        taskTitle = j['task_title'],
        agent = j['agent'],
        summary = j['summary'],
        reason = j['reason'],
        createdLabel = j['created_label'];
}

class ProjectView {
  final int id;
  final String name;
  final String policy; // NONE, OWN, ALL
  final int shelf, cart, done;

  ProjectView.fromJson(Map<String, dynamic> j)
      : id = j['id'],
        name = j['name'],
        policy = j['policy'],
        shelf = j['shelf'],
        cart = j['cart'],
        done = j['done'];
}

class Board {
  final List<TaskView> shelf, cart, done;
  final List<SessionView> sessions;
  final List<SuggestionView> suggestions;
  final List<ProjectView> projects;
  final int windowMinutes;

  Board.fromJson(Map<String, dynamic> j)
      : shelf = _list(j['shelf'], TaskView.fromJson),
        cart = _list(j['cart'], TaskView.fromJson),
        done = _list(j['done'], TaskView.fromJson),
        sessions = _list(j['sessions'], SessionView.fromJson),
        suggestions = _list(j['suggestions'], SuggestionView.fromJson),
        projects = _list(j['projects'], ProjectView.fromJson),
        windowMinutes = j['window_minutes'] ?? 10;

  List<TaskView> tasks(String status) => switch (status) { 'SHELF' => shelf, 'CART' => cart, _ => done };
}

class HistoryView {
  final String title;
  final bool daily;
  final String date;
  final String? project;
  final String? agent;

  HistoryView.fromJson(Map<String, dynamic> j)
      : title = j['title'],
        daily = j['daily'],
        date = j['date'],
        project = j['project'],
        agent = j['agent'];
}

class Peer {
  final String device;
  final String name;
  final String url;
  final String? lastSyncedAt;

  Peer.fromJson(Map<String, dynamic> j)
      : device = j['device'],
        name = j['name'],
        url = j['url'],
        lastSyncedAt = j['last_synced_at'];

  String get primaryUrl => url.split(' ').first;

  /// "há 3 min", a partir de `last_synced_at` (hora local, "AAAA-MM-DD HH:MM:SS").
  String get lastSyncedLabel {
    final at = DateTime.tryParse(lastSyncedAt?.replaceFirst(' ', 'T') ?? '');
    if (at == null) return 'nunca sincronizado';
    final minutes = DateTime.now().difference(at).inMinutes;
    if (minutes < 1) return 'sincronizado agora';
    if (minutes < 60) return 'sincronizado há $minutes min';
    if (minutes < 60 * 24) return 'sincronizado há ${minutes ~/ 60} h';
    return 'sincronizado há ${minutes ~/ (60 * 24)} dias';
  }
}

List<T> _list<T>(dynamic raw, T Function(Map<String, dynamic>) parse) =>
    (raw as List).map((e) => parse(e as Map<String, dynamic>)).toList();
