import { mount } from 'svelte';
import App from './app/App.svelte';
import './styles/tokens.css';
import './styles/base.css';

const target = document.getElementById('app');
if (!target) throw new Error('#app is missing from index.html');

const app = mount(App, { target });

// The mark index.html paints while the bundle loads. Removed rather than hidden: it is the
// topmost thing on the page and would otherwise swallow every click.
document.getElementById('boot')?.remove();

export default app;
