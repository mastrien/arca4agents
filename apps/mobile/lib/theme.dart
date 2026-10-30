import 'package:flutter/material.dart';

/// Cores do Arca4agents (as mesmas do app desktop): verde-petróleo para o carrinho e as ações,
/// âmbar só para o que pede atenção, índigo para o que foi criado por agentes.
@immutable
class ArcaColors extends ThemeExtension<ArcaColors> {
  final Color ground, surface, recessed, line, lineSoft, text, text2, muted;
  final Color accent, accentTint, accentLine, ok, attention, attentionDot, attentionTint, agent, agentTint, idle;

  const ArcaColors({
    required this.ground,
    required this.surface,
    required this.recessed,
    required this.line,
    required this.lineSoft,
    required this.text,
    required this.text2,
    required this.muted,
    required this.accent,
    required this.accentTint,
    required this.accentLine,
    required this.ok,
    required this.attention,
    required this.attentionDot,
    required this.attentionTint,
    required this.agent,
    required this.agentTint,
    required this.idle,
  });

  static const light = ArcaColors(
    ground: Color(0xFFF3F4F1),
    surface: Color(0xFFFFFFFF),
    recessed: Color(0xFFE6E8E3),
    line: Color(0xFFE1E4DF),
    lineSoft: Color(0xFFEEF0EC),
    text: Color(0xFF1B1F1D),
    text2: Color(0xFF3D433F),
    muted: Color(0xFF5A615C),
    accent: Color(0xFF17654F),
    accentTint: Color(0xFFE2F0EA),
    accentLine: Color(0xFF9FC7B6),
    ok: Color(0xFF1F8A63),
    attention: Color(0xFF8A4209),
    attentionDot: Color(0xFFC2691B),
    attentionTint: Color(0xFFFCEFDF),
    agent: Color(0xFF3B4AA0),
    agentTint: Color(0xFFE9ECFA),
    idle: Color(0xFF8A918C),
  );

  static const dark = ArcaColors(
    ground: Color(0xFF141715),
    surface: Color(0xFF1C201E),
    recessed: Color(0xFF232825),
    line: Color(0xFF2D332F),
    lineSoft: Color(0xFF252A27),
    text: Color(0xFFE8EBE7),
    text2: Color(0xFFC5CBC6),
    muted: Color(0xFF9AA29C),
    accent: Color(0xFF5CC29B),
    accentTint: Color(0xFF1D3229),
    accentLine: Color(0xFF2F5A49),
    ok: Color(0xFF5CC29B),
    attention: Color(0xFFF0B27A),
    attentionDot: Color(0xFFE08A3C),
    attentionTint: Color(0xFF3A2A1A),
    agent: Color(0xFFA9B4F0),
    agentTint: Color(0xFF262B45),
    idle: Color(0xFF6F7771),
  );

  @override
  ArcaColors copyWith() => this;

  @override
  ArcaColors lerp(ArcaColors? other, double t) => t < 0.5 || other == null ? this : other;
}

extension ArcaTheme on BuildContext {
  ArcaColors get arca => Theme.of(this).extension<ArcaColors>()!;
}

const mono = 'IBM Plex Mono';

ThemeData arcaTheme(Brightness brightness) {
  final c = brightness == Brightness.light ? ArcaColors.light : ArcaColors.dark;
  final onAccent = brightness == Brightness.light ? Colors.white : const Color(0xFF0D1A15);
  final scheme = ColorScheme.fromSeed(seedColor: c.accent, brightness: brightness).copyWith(
    primary: c.accent,
    onPrimary: onAccent,
    surface: c.ground,
    onSurface: c.text,
    onSurfaceVariant: c.muted,
    outline: c.line,
    outlineVariant: c.lineSoft,
  );
  final radius = BorderRadius.circular(12);
  return ThemeData(
    useMaterial3: true,
    brightness: brightness,
    colorScheme: scheme,
    fontFamily: 'IBM Plex Sans',
    scaffoldBackgroundColor: c.ground,
    extensions: [c],
    appBarTheme: AppBarTheme(backgroundColor: c.ground, foregroundColor: c.text, elevation: 0, scrolledUnderElevation: 0),
    dividerTheme: DividerThemeData(color: c.lineSoft, space: 1),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        backgroundColor: c.accent,
        foregroundColor: onAccent,
        minimumSize: const Size(64, 48),
        shape: RoundedRectangleBorder(borderRadius: radius),
        textStyle: const TextStyle(fontFamily: 'IBM Plex Sans', fontSize: 15, fontWeight: FontWeight.w600),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: c.text,
        backgroundColor: c.surface,
        minimumSize: const Size(64, 48),
        side: BorderSide(color: c.line),
        shape: RoundedRectangleBorder(borderRadius: radius),
        textStyle: const TextStyle(fontFamily: 'IBM Plex Sans', fontSize: 15),
      ),
    ),
    textButtonTheme: TextButtonThemeData(style: TextButton.styleFrom(foregroundColor: c.text2)),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: c.surface,
      contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
      border: OutlineInputBorder(borderRadius: radius, borderSide: BorderSide(color: c.line)),
      enabledBorder: OutlineInputBorder(borderRadius: radius, borderSide: BorderSide(color: c.line)),
      focusedBorder: OutlineInputBorder(borderRadius: radius, borderSide: BorderSide(color: c.accent, width: 2)),
    ),
    bottomSheetTheme: BottomSheetThemeData(
      backgroundColor: c.surface,
      shape: const RoundedRectangleBorder(borderRadius: BorderRadius.vertical(top: Radius.circular(22))),
      dragHandleColor: c.line,
    ),
    navigationBarTheme: NavigationBarThemeData(
      backgroundColor: c.surface,
      indicatorColor: c.accentTint,
      surfaceTintColor: Colors.transparent,
      labelTextStyle: WidgetStateProperty.resolveWith((states) => TextStyle(
            fontSize: 12,
            fontWeight: states.contains(WidgetState.selected) ? FontWeight.w600 : FontWeight.w400,
            color: states.contains(WidgetState.selected) ? c.accent : c.text2,
          )),
      iconTheme: WidgetStateProperty.resolveWith((states) => IconThemeData(color: states.contains(WidgetState.selected) ? c.accent : c.text2)),
    ),
    floatingActionButtonTheme: FloatingActionButtonThemeData(
      backgroundColor: c.accent,
      foregroundColor: onAccent,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(18)),
    ),
    snackBarTheme: const SnackBarThemeData(behavior: SnackBarBehavior.floating),
  );
}

// Cor estável por projeto, derivada do nome (o app desktop usa a mesma conta).
const _projectColors = [
  Color(0xFF3B6FD8),
  Color(0xFFC2691B),
  Color(0xFF7A4FC4),
  Color(0xFF1F8A63),
  Color(0xFFC23B6B),
  Color(0xFF2A8C9C),
  Color(0xFF8C6A1F),
  Color(0xFF5A615C),
];

Color projectColor(String name) {
  var hash = 0;
  for (final unit in name.codeUnits) {
    hash = (hash * 31 + unit) & 0x7fffffff;
  }
  return _projectColors[hash % _projectColors.length];
}
