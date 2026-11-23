import type { FC } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { Container } from '@mantine/core';
import {
  action,
  binding,
  intro,
  introLede,
  key,
  layout,
  list,
  scriptingBody,
  scriptingTitle,
} from '@/components/Keybindings/Keybindings.css';
import { Commands } from '@/components/shared/Commands';
import { body, CONTAINER, heading, lede, section, wrap } from '@/components/shared/shared.css';
import { links } from '@/lib/links';

const BINDINGS = [
  { keys: 'Enter', id: 'password' },
  { keys: 'Ctrl-U', id: 'username' },
  { keys: 'Ctrl-T', id: 'totp' },
  { keys: 'Ctrl-O', id: 'uri' },
  { keys: 'Ctrl-N', id: 'notes' },
  { keys: 'Ctrl-R', id: 'sync' },
  { keys: 'Alt-N', id: 'add' },
  { keys: 'Alt-E', id: 'edit' },
  { keys: 'Tab', id: 'profile' },
  { keys: 'Esc', id: 'quit' },
] as const;

const SCRIPTS = [
  'pass=$(bitpick --print)',
  'bitpick --field totp --print',
  'bitpick --query github --sync',
];

export const Keybindings: FC = () => {
  const { t } = useTranslation();

  return (
    <Container
      component="section"
      id="keys"
      size={CONTAINER}
      className={`${wrap} ${section} ${layout}`}
    >
      <div className={intro}>
        <h2 className={heading}>{t('keybindings.title')}</h2>
        <p className={`${lede} ${introLede}`}>
          <Trans i18nKey="keybindings.lede" components={{ link: <a href={links.keybindings} /> }} />
        </p>
        <h3 className={scriptingTitle}>{t('keybindings.scripting')}</h3>
        <p className={`${body} ${scriptingBody}`}>{t('keybindings.scriptingBody')}</p>
        <Commands lines={SCRIPTS} />
      </div>
      <dl className={list}>
        {BINDINGS.map(({ keys, id }) => (
          <div key={id} className={binding}>
            <dt>
              <kbd className={key}>{keys}</kbd>
            </dt>
            <dd className={action}>{t(`keybindings.bindings.${id}`)}</dd>
          </div>
        ))}
      </dl>
    </Container>
  );
};
