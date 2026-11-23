import { style } from '@vanilla-extract/css';
import { colors } from '@/theme/colors';

export const hero = style({
  display: 'flex',
  flexDirection: 'column',
  gap: 56,
  paddingTop: 88,
  paddingBottom: 96,
});

export const copy = style({ maxWidth: 760, minWidth: 0 });

export const title = style({
  marginBottom: 24,
  fontSize: 'clamp(40px, 6vw, 64px)',
  lineHeight: 1.05,
  fontWeight: 600,
  letterSpacing: '-0.02em',
});

export const heroLede = style({
  maxWidth: 560,
  marginBottom: 36,
  fontSize: 19,
  color: colors.soft,
});

export const ctas = style({ display: 'flex', flexWrap: 'wrap', gap: 12, marginBottom: 28 });

export const note = style({ fontSize: 14, color: colors.muted });

export const screenColumn = style({ minWidth: 0 });
