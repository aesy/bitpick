import { globalStyle } from '@vanilla-extract/css';
import { colors } from '@/theme/colors';
import { vars } from '@/theme/theme-vars';

globalStyle('html', {
  scrollBehavior: 'smooth',
  '@media': {
    '(prefers-reduced-motion: reduce)': { scrollBehavior: 'auto' },
  },
});

globalStyle('body', {
  margin: 0,
  fontFamily: vars.fontFamily,
  fontSize: 16,
  lineHeight: 1.6,
  color: colors.text,
  background: colors.page,
  WebkitFontSmoothing: 'antialiased',
});

globalStyle('h1, h2, h3, p, dl, dd, ul, figure', { margin: 0, padding: 0 });

globalStyle('code, kbd', { fontFamily: vars.fontFamilyMonospace });

globalStyle('a', { color: colors.accent, textDecoration: 'none' });
globalStyle('a:hover', { color: colors.accentHover });

globalStyle('a:focus-visible, button:focus-visible', {
  outline: `2px solid ${colors.accent}`,
  outlineOffset: 3,
});
