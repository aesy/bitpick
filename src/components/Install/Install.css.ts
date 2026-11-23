import { globalStyle, style } from '@vanilla-extract/css';
import { colors } from '@/theme/colors';
import { vars } from '@/theme/theme-vars';

export const installLede = style({ maxWidth: 680, marginBottom: 40 });

export const grid = style({
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(min(100%, 300px), 1fr))',
  gap: 20,
});

export const card = style({
  display: 'flex',
  flexDirection: 'column',
  gap: 14,
  padding: 28,
  border: `1px solid ${colors.line}`,
  borderRadius: 14,
  background: colors.surface,
});

export const cardHeading = style({ fontSize: 19, fontWeight: 600 });

export const releasesButton = style({ alignSelf: 'flex-start', marginTop: 'auto', minHeight: 44 });

export const snippet = style({
  display: 'block',
  marginTop: 'auto',
  padding: '12px 14px',
  borderRadius: 8,
  background: colors.page,
  fontFamily: vars.fontFamilyMonospace,
  fontSize: 14,
  lineHeight: 1.7,
});

globalStyle(`${snippet} > span`, { whiteSpace: 'normal', overflowWrap: 'anywhere' });

export const firstRun = style({
  display: 'flex',
  flexWrap: 'wrap',
  alignItems: 'baseline',
  gap: '12px 32px',
  marginTop: 40,
  padding: '24px 28px',
  border: `1px dashed ${colors.lineStrong}`,
  borderRadius: 14,
});

export const firstRunTitle = style({ fontSize: 17, fontWeight: 600 });

export const firstRunCommands = style({
  display: 'block',
  minWidth: 0,
  fontFamily: vars.fontFamilyMonospace,
  fontSize: 14,
  lineHeight: 1.9,
  overflowX: 'auto',
});
