import { style } from '@vanilla-extract/css';
import { colors } from '@/theme/colors';
import { vars } from '@/theme/theme-vars';

export const CONTAINER = 1360;

export const page = style({
  overflowX: 'clip',
  minHeight: '100vh',
  background: colors.page,
  color: colors.text,
});

// Doubled so it beats Mantine's own Container padding.
export const wrap = style({
  width: '100%',
  boxSizing: 'border-box',
  selectors: { '&&': { paddingInline: 24 } },
});

export const section = style({ paddingBlock: 96 });

export const band = style({
  borderTop: `1px solid ${colors.lineSoft}`,
  borderBottom: `1px solid ${colors.lineSoft}`,
  background: colors.band,
});

export const heading = style({
  marginBottom: 12,
  fontSize: 'clamp(30px, 4vw, 40px)',
  lineHeight: 1.15,
  fontWeight: 600,
  letterSpacing: '-0.01em',
});

export const lede = style({ fontSize: 18, color: colors.soft });

export const cardTitle = style({ marginBottom: 8, fontSize: 19, fontWeight: 600 });

export const body = style({ color: colors.soft });

export const button = style({
  display: 'inline-flex',
  alignItems: 'center',
  boxSizing: 'border-box',
  minHeight: 48,
  paddingInline: 24,
  borderRadius: 10,
  fontWeight: 500,
  textDecoration: 'none',
});

export const primaryButton = style([
  button,
  {
    background: colors.primary,
    color: colors.onPrimary,
    fontWeight: 600,
    ':hover': { color: colors.onPrimary, filter: 'brightness(1.12)' },
  },
]);

export const outlineButton = style([
  button,
  {
    border: `1px solid ${colors.lineStrong}`,
    color: colors.text,
    ':hover': { color: colors.text, borderColor: colors.keyBorder },
  },
]);

export const prompt = style({ color: colors.accent });

export const commands = style({
  display: 'block',
  padding: '18px 20px',
  border: `1px solid ${colors.line}`,
  borderRadius: 12,
  background: colors.surface,
  fontFamily: vars.fontFamilyMonospace,
  fontSize: 14,
  lineHeight: 1.9,
  overflowX: 'auto',
});

export const command = style({ display: 'block', whiteSpace: 'nowrap' });
