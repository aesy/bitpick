import { style, styleVariants } from '@vanilla-extract/css';
import { colors, terminal } from '@/theme/colors';
import { vars } from '@/theme/theme-vars';

export const frame = style({
  border: `1px solid ${terminal.frame}`,
  borderRadius: 14,
  background: colors.surface,
  boxShadow: '0 30px 80px rgba(0, 0, 0, 0.45)',
  overflow: 'hidden',
});

export const titleBar = style({
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  padding: '12px 16px',
  borderBottom: `1px solid ${colors.line}`,
  background: colors.surfaceRaised,
});

export const dot = style({ width: 12, height: 12, borderRadius: '50%', background: terminal.dot });

export const windowTitle = style({
  marginLeft: 12,
  fontFamily: vars.fontFamilyMonospace,
  fontSize: 13,
  color: colors.muted,
});

export const scroller = style({ overflowX: 'auto' });

export const screenArea = style({
  minWidth: 680,
  padding: '16px 12px 10px',
  display: 'flex',
  flexDirection: 'column',
  gap: 18,
  fontFamily: vars.fontFamilyMonospace,
  fontSize: 14,
  lineHeight: 1.6,
  color: colors.text,
});

export const line = style({ whiteSpace: 'pre' });

export const pane = style({
  position: 'relative',
  border: '1px solid',
  borderRadius: 6,
  padding: '14px 10px 12px',
});

export const paneBorder = styleVariants({
  blue: { borderColor: terminal.blue },
  magenta: { borderColor: terminal.magenta },
  green: { borderColor: terminal.green },
  yellow: { borderColor: terminal.yellow },
});

// A title sits on the border row like ratatui draws it, so it stays one text line high.
const paneTitleBase = style({
  position: 'absolute',
  left: 8,
  lineHeight: 1,
  whiteSpace: 'pre',
  background: colors.surface,
});

export const paneTitle = style([paneTitleBase, { top: -8 }]);

export const paneTitleBottom = style([paneTitleBase, { bottom: -8 }]);

export const listPane = style({ paddingInline: 0, paddingBottom: 10 });

export const chips = style({ display: 'flex', gap: 8, marginTop: 2 });

export const chip = style({ whiteSpace: 'pre', background: terminal.chip, fontWeight: 700 });

export const cursor = style({
  display: 'inline-block',
  width: 8,
  height: 16,
  verticalAlign: -3,
  background: colors.text,
});

export const middle = style({
  display: 'grid',
  gridTemplateColumns: '56fr 44fr',
  gap: 18,
});

export const side = style({ display: 'flex', flexDirection: 'column', gap: 18 });

export const grow = style({ flex: 1 });

export const item = style({ paddingInline: 10 });

export const selectedItem = style({
  paddingInline: 10,
  background: terminal.highlight,
  color: terminal.onHighlight,
  fontWeight: 700,
});

export const bold = style({ fontWeight: 700 });

export const fg = styleVariants({
  cyan: { color: terminal.cyan },
  yellow: { color: terminal.yellow },
  magenta: { color: terminal.magenta },
  gray: { color: terminal.gray },
});
