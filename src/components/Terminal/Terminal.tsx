import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { screen } from '@/components/Terminal/screen';
import {
  bold,
  chip,
  chips,
  cursor,
  dot,
  fg,
  frame,
  grow,
  item,
  line,
  listPane,
  middle,
  pane,
  paneBorder,
  paneTitle,
  paneTitleBottom,
  screenArea,
  scroller,
  selectedItem,
  side,
  titleBar,
  windowTitle,
} from '@/components/Terminal/Terminal.css';

const Title: FC<{ text: string }> = ({ text }) => <span className={paneTitle}>{` ${text} `}</span>;

export const Terminal: FC = () => {
  const { t } = useTranslation();
  const [selected, ...rest] = screen.items;

  return (
    <figure className={frame} aria-label={t('hero.screen')}>
      <div className={titleBar}>
        <span aria-hidden="true" className={dot} />
        <span aria-hidden="true" className={dot} />
        <span aria-hidden="true" className={dot} />
        <span className={windowTitle}>{screen.title}</span>
      </div>
      <div className={scroller}>
        <div className={screenArea}>
          <div className={`${pane} ${paneBorder.blue}`}>
            <Title text={screen.panes.overview} />
            <div className={line}>
              <span className={`${fg.cyan} ${bold}`}>{screen.app}</span>
              {'  '}
              <span className={fg.gray}>{screen.profileLabel}</span>
              <span className={`${fg.yellow} ${bold}`}>{screen.profile}</span>
              <span className={fg.gray}>{screen.profileCount}</span>
            </div>
            <div className={chips}>
              {screen.chips.map(text => (
                <span key={text} className={chip}>{` ${text} `}</span>
              ))}
            </div>
          </div>

          <div className={`${pane} ${paneBorder.magenta}`}>
            <Title text={screen.panes.search} />
            <div className={line}>
              {screen.query}
              <span aria-hidden="true" className={cursor} />
            </div>
            <span className={paneTitleBottom}>{screen.searchHint}</span>
          </div>

          <div className={middle}>
            <div className={`${pane} ${listPane} ${paneBorder.blue}`}>
              <Title text={screen.listTitle} />
              <div className={selectedItem}>
                <div className={line}>{`❯ ${selected.name}  [${selected.folder}]`}</div>
                <div className={line}>{`  ${selected.secondary}`}</div>
              </div>
              {rest.map(entry => (
                <div key={entry.name} className={item}>
                  <div className={line}>
                    {'  '}
                    <span className={bold}>{entry.name}</span>
                    {entry.note && (
                      <>
                        {'  '}
                        <span className={fg.yellow}>{screen.noteTag}</span>
                      </>
                    )}
                    {'  '}
                    <span className={fg.magenta}>[{entry.folder}]</span>
                  </div>
                  <div className={`${line} ${fg.gray}`}>{`  ${entry.secondary}`}</div>
                </div>
              ))}
            </div>

            <div className={side}>
              <div className={`${pane} ${grow} ${paneBorder.green}`}>
                <Title text={screen.panes.details} />
                {screen.details.map(([label, value]) => (
                  <div key={label} className={line}>
                    <span className={`${fg.cyan} ${bold}`}>{label}</span>
                    {value}
                  </div>
                ))}
                <div className={line}> </div>
                <div className={line}>
                  <span className={`${fg.yellow} ${bold}`}>{screen.defaultAction[0]}</span>
                  {screen.defaultAction[1]}
                </div>
              </div>
              <div className={`${pane} ${paneBorder.yellow}`}>
                <Title text={screen.panes.actions} />
                {screen.actions.map(text => (
                  <div key={text} className={line}>
                    {text}
                  </div>
                ))}
              </div>
            </div>
          </div>

          <div className={`${line} ${fg.cyan}`}>{screen.status}</div>
        </div>
      </div>
    </figure>
  );
};
