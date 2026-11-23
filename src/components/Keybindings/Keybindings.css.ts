import { style } from '@vanilla-extract/css';
import { colors } from '@/theme/colors';

export const layout = style({ display: 'flex', flexWrap: 'wrap', gap: 56 });

export const intro = style({ flex: '1 1 340px', minWidth: 0 });

export const introLede = style({ marginBottom: 28 });

export const scriptingTitle = style({ marginBottom: 12, fontSize: 19, fontWeight: 600 });

export const scriptingBody = style({ marginBottom: 16 });

export const list = style({
  flex: '1 1 480px',
  minWidth: 0,
  margin: 0,
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(min(100%, 220px), 1fr))',
  alignContent: 'start',
  gap: 12,
});

export const binding = style({
  display: 'flex',
  alignItems: 'center',
  gap: 14,
  padding: '14px 16px',
  border: `1px solid ${colors.line}`,
  borderRadius: 12,
  background: colors.surface,
});

export const key = style({
  display: 'inline-block',
  minWidth: 64,
  whiteSpace: 'nowrap',
  padding: '2px 8px',
  border: `1px solid ${colors.keyBorder}`,
  borderBottomWidth: 3,
  borderRadius: 6,
  background: colors.keycap,
  color: colors.text,
  fontSize: 13,
  textAlign: 'center',
});

export const action = style({ color: colors.label });
