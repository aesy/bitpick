import { style } from '@vanilla-extract/css';

export const layout = style({
  display: 'flex',
  flexWrap: 'wrap',
  gap: '40px 56px',
  paddingBlock: 80,
});

export const intro = style({ flex: '1 1 300px', minWidth: 0 });

export const list = style({
  flex: '2 1 520px',
  minWidth: 0,
  listStyle: 'none',
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(min(100%, 240px), 1fr))',
  gap: 28,
});

export const itemTitle = style({ marginBottom: 6, fontSize: 17, fontWeight: 600 });
