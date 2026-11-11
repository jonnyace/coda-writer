import { mount } from 'svelte'
import '@fontsource-variable/literata/opsz.css'
import '@fontsource-variable/literata/opsz-italic.css'
import '@fontsource-variable/schibsted-grotesk'
import './app.css'
import App from './App.svelte'

export default mount(App, { target: document.getElementById('app')! })
