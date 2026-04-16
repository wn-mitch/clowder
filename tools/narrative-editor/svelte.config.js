/** @type {import("@sveltejs/vite-plugin-svelte").SvelteConfig} */
export default {
  warningFilter: (warning) => {
    // Suppress a11y label warnings — this is a dev tool, not a public-facing app
    if (warning.code === 'a11y_label_has_associated_control') return false
    return true
  },
}
