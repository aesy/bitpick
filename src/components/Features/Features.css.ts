import { style } from '@vanilla-extract/css';
import { colors } from '@/theme/colors';

export const featuresLede = style({ maxWidth: 640, marginBottom: 48 });

export const grid = style({
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(min(100%, 300px), 1fr))',
  gap: 20,
});

export const card = style({
  padding: 28,
  border: `1px solid ${colors.line}`,
  borderRadius: 14,
  background: colors.page,
});

export const icon = style({ display: 'block', marginBottom: 16 });
