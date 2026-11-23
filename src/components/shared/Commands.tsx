import type { FC } from 'react';
import { command, commands as commandsBox, prompt } from '@/components/shared/shared.css';

type Props = {
  lines: readonly string[];
  className?: string;
};

export const Commands: FC<Props> = ({ lines, className }) => (
  <code className={className ?? commandsBox}>
    {lines.map(line => (
      <span key={line} className={command}>
        <span className={prompt}>$ </span>
        {line}
      </span>
    ))}
  </code>
);
