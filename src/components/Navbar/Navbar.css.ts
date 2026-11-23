import { style } from '@vanilla-extract/css';
import { colors } from '@/theme/colors';
import { vars } from '@/theme/theme-vars';

export const navbar = style({ borderBottom: `1px solid ${colors.line}` });

export const nav = style({
  display: 'flex',
  flexWrap: 'wrap',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 16,
  paddingBlock: 18,
});

export const brand = style({
  display: 'flex',
  alignItems: 'center',
  gap: 10,
  color: colors.text,
  fontFamily: vars.fontFamilyMonospace,
  fontWeight: 700,
  fontSize: 20,
  ':hover': { color: colors.text },
});

export const brandLogo = style({ display: 'block' });

export const actions = style({
  display: 'flex',
  flexWrap: 'wrap',
  alignItems: 'center',
  gap: '8px 24px',
  fontSize: 15,
});

export const link = style({
  color: colors.nav,
  ':hover': { color: colors.text },
  '@media': { '(max-width: 560px)': { display: 'none' } },
});

export const githubButton = style({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 8,
  minHeight: 44,
  paddingInline: 16,
  border: `1px solid ${colors.lineStrong}`,
  borderRadius: 10,
  color: colors.text,
  ':hover': { color: colors.text, borderColor: colors.keyBorder },
});
