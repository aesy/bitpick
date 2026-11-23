import { render, screen, within } from '@testing-library/react';
import { MantineProvider } from '@mantine/core';
import App from '@/App';
import { initI18n } from '@/i18n';
import { links } from '@/lib/links';
import { theme } from '@/theme/theme';

const renderApp = () =>
  render(
    <MantineProvider theme={theme} forceColorScheme="dark" env="test">
      <App />
    </MantineProvider>
  );

beforeAll(() => initI18n());

describe('App', () => {
  it('shows the picker as the TUI draws it', () => {
    renderApp();
    const picker = screen.getByRole('figure', {
      name: 'The bitpick picker filtering a vault for git',
    });

    for (const pane of ['Overview', 'Search', 'Details', 'Actions']) {
      expect(within(picker).getByText(pane)).toBeInTheDocument();
    }
    expect(within(picker).getByText('Credentials · Enter copies password')).toBeInTheDocument();
  });

  it('links the hero to the install section and the source', () => {
    renderApp();

    expect(screen.getAllByRole('link', { name: 'Install' })[0]).toHaveAttribute('href', '#install');
    expect(screen.getByRole('link', { name: 'Source on GitHub' })).toHaveAttribute(
      'href',
      links.source
    );
  });

  it('offers release binaries before building from source', () => {
    renderApp();
    const headings = screen
      .getAllByRole('heading', { level: 3 })
      .map(heading => heading.textContent);

    expect(headings.indexOf('Release binaries')).toBeLessThan(headings.indexOf('Arch Linux'));
    expect(screen.getAllByRole('link', { name: 'Releases' })[0]).toHaveAttribute(
      'href',
      links.releases
    );
  });
});
