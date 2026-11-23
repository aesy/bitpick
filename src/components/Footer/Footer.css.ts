import { style } from '@vanilla-extract/css';
import { colors } from '@/theme/colors';
import { vars } from '@/theme/theme-vars';

export const footer = style({ borderTop: `1px solid ${colors.line}` });

export const row = style({
  display: 'flex',
  flexWrap: 'wrap',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 20,
  paddingTop: 36,
  paddingBottom: 48,
  fontSize: 14,
  color: colors.muted,
});

export const about = style({ display: 'flex', flexDirection: 'column', gap: 6, minWidth: 0 });

export const brand = style({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 8,
  fontFamily: vars.fontFamilyMonospace,
  fontWeight: 700,
  color: colors.text,
});

export const links = style({
  display: 'flex',
  flexWrap: 'wrap',
  alignItems: 'center',
  gap: '8px 24px',
});

export const link = style({
  display: 'inline-flex',
  alignItems: 'center',
  minHeight: 44,
  color: colors.nav,
  ':hover': { color: colors.text },
});
