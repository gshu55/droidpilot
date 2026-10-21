<script setup lang="ts">
import { getTauriVersion } from "@tauri-apps/api/app";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { computed, onMounted, ref } from "vue";
import { useI18n } from "../i18n";
import { useAppUpdate } from "../composables/useAppUpdate";

const { t, language } = useI18n();
const { state, busy, percent, check, install } = useAppUpdate();
const appVersion = ref(__APP_VERSION__);
const tauriVersion = ref("");
const linkError = ref("");
const confirming = ref(false);
const desktop = isTauri();
const repository = "https://github.com/gshu55/droidpilot";
const checkedTime = computed(() => state.checkedAt?.toLocaleString(language.value, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }));
const progressLabel = computed(() => {
  const downloaded = (state.downloaded / 1024 / 1024).toFixed(1);
  return state.total ? `${downloaded} / ${(state.total / 1024 / 1024).toFixed(1)} MB` : `${downloaded} MB`;
});
const errorMessage = computed(() => t(`about.updateError.${["check", "timeout", "download", "signature", "install", "busy", "missing"].includes(state.errorCode) ? state.errorCode : "unknown"}`));
const statusTitle = computed(() => state.update && ["available", "error"].includes(state.phase)
  ? t("about.newVersion", { version: state.update.version }) : t(`about.phase.${state.phase}`));

async function visit(path = "") {
  linkError.value = "";
  try { await openUrl(`${repository}${path}`); }
  catch { linkError.value = t("about.linkError"); }
}

async function confirmInstall() {
  if (busy.value || confirming.value) return;
  confirming.value = true;
  try {
    if (await ask(t("about.installPrompt"), {
      title: t("about.installTitle"), kind: "info",
      okLabel: t("about.downloadInstall"), cancelLabel: t("about.later"),
    })) await install();
  } catch { linkError.value = t("about.dialogError"); }
  finally { confirming.value = false; }
}

onMounted(async () => {
  if (!desktop) return;
  const [app, tauri] = await Promise.allSettled([invoke<string>("get_cargo_package_version"), getTauriVersion()]);
  if (app.status === "fulfilled" && app.value.trim()) appVersion.value = app.value;
  if (tauri.status === "fulfilled") tauriVersion.value = tauri.value;
});
</script>

<template>
  <section class="card panel about-page" :aria-label="t('common.about')">
    <header class="product-header">
      <div class="product-mark" aria-hidden="true">
        <svg viewBox="0 0 40 40" fill="none"><rect x="11" y="5" width="18" height="30" rx="5"/><path d="M17 10h6M18 30h4M6 16l-4 4 4 4m28-8 4 4-4 4"/></svg>
      </div>
      <div class="product-heading">
        <div class="product-title"><h1>DroidPilot</h1><span class="version-tag">v{{ appVersion }}</span></div>
        <p>{{ t("about.subtitle") }}</p>
      </div>
    </header>

    <article class="update-panel" :class="{ 'has-update': state.update, 'has-error': state.phase === 'error' }" :aria-busy="busy">
      <div class="update-top">
        <div><span class="section-kicker">{{ t("about.softwareUpdate") }}</span>
          <h2 aria-live="polite">{{ statusTitle }}</h2>
          <p class="update-description">{{ t('about.updateSource') }}</p>
        </div>
        <div class="update-actions">
          <button v-if="state.update" type="button" class="primary-action" :disabled="busy || confirming" @click="confirmInstall">{{ t(state.phase === 'error' ? 'about.retryInstall' : 'about.downloadInstall') }}</button>
          <button type="button" :class="state.update ? 'secondary-action' : 'primary-action'" :disabled="!desktop || busy || confirming" @click="check">
            <svg v-if="state.phase === 'checking'" class="spinner" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5"/></svg>
            {{ t(state.phase === 'checking' ? 'about.checking' : 'about.checkUpdate') }}
          </button>
        </div>
      </div>
      <div v-if="state.phase === 'downloading' || state.phase === 'installing'" class="download-progress" aria-live="polite">
        <progress :value="percent ?? undefined" max="100" :aria-label="t('about.downloadProgress')" />
        <div class="progress-meta"><span>{{ progressLabel }}<template v-if="state.attempt > 1"> · {{ t('about.retryAttempt', { attempt: state.attempt }) }}</template></span><span>{{ percent === null ? t('about.connecting') : `${percent}%` }}</span></div>
      </div>
      <div v-if="state.phase === 'error'" class="update-error" role="alert">
        <p>{{ errorMessage }}</p>
        <details><summary>{{ t('about.errorDetails') }}</summary><pre>{{ state.errorDetail }}</pre></details>
      </div>
      <div v-if="state.update?.notes" class="release-notes"><h3>{{ t('about.releaseNotes') }}</h3><pre>{{ state.update.notes }}</pre></div>
      <footer class="update-footer">
        <span v-if="!desktop">{{ t('about.desktopOnly') }}</span>
        <span v-else-if="checkedTime">{{ t('about.lastChecked', { time: checkedTime }) }}</span>
        <span v-else>{{ t('about.manualCheck') }}</span>
        <button type="button" class="text-action" @click="visit('/releases')">{{ t('about.allReleases') }} <span aria-hidden="true">↗</span></button>
      </footer>
    </article>

    <nav class="project-links" :aria-label="t('about.projectLinks')">
      <button type="button" @click="visit()"><span>{{ t('about.sourceCode') }}</span><span aria-hidden="true">↗</span></button>
      <button type="button" @click="visit('/issues')"><span>{{ t('about.feedback') }}</span><span aria-hidden="true">↗</span></button>
    </nav>
    <p v-if="linkError" class="link-error" role="alert">{{ linkError }}</p>
    <div class="about-details">
      <section class="info-section"><h3>{{ t('about.privacy') }}</h3><p>{{ t('about.privacySummary') }}</p></section>
      <details class="info-section"><summary>{{ t('about.statement') }}</summary><p>{{ t('about.usageSummary') }}</p></details>
      <details class="info-section"><summary>{{ t('about.oss') }}</summary><p>{{ t('about.ossSummary') }}</p></details>
    </div>
    <footer class="technical-footer"><span>{{ t('about.runtimeLocal') }}</span><span v-if="tauriVersion">Tauri {{ tauriVersion }}</span><span>scrcpy 4.1</span></footer>
  </section>
</template>

<style scoped>
.about-page { width: 100%; max-width: 860px; margin: 0 auto; color: var(--text-main); }
.product-header { display: flex; align-items: center; gap: 16px; padding: 12px 0 24px; }
.product-mark { display: grid; place-items: center; width: 60px; height: 60px; flex: 0 0 auto; border-radius: 16px; background: var(--primary-soft); color: var(--primary); border: 1px solid var(--primary-border); }
.product-mark svg { width: 38px; stroke: currentColor; stroke-width: 1.7; stroke-linecap: round; stroke-linejoin: round; }
.product-heading { min-width: 0; }
.product-title { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
.product-title h1 { margin: 0; font-size: 25px; font-weight: 650; letter-spacing: -.8px; line-height: 1.3; }
.version-tag { font: 11px var(--font-code); border: 1px solid var(--border); border-radius: 5px; padding: 3px 7px; color: var(--text-sub); }
.product-heading p { margin: 6px 0 0; font-size: 12px; color: var(--text-sub); line-height: 1.6; }
.update-panel { border: 1px solid var(--border); border-radius: 12px; overflow: hidden; background: var(--surface-solid); }
.update-panel.has-update { border-color: var(--primary-border); }
.update-panel.has-error { border-color: var(--danger-border); }
.update-top { padding: 20px; display: flex; justify-content: space-between; gap: 20px; align-items: center; }
.section-kicker { font-size: 11px; color: var(--text-sub); letter-spacing: .06em; }
.update-top h2 { margin: 7px 0; font-size: 17px; line-height: 1.4; font-weight: 600; }
.update-description { margin: 0; color: var(--text-sub); font-size: 12px; line-height: 1.6; }
.update-actions { display: flex; flex: 0 0 auto; align-items: center; gap: 8px; flex-wrap: wrap; }
button { font-family: inherit; cursor: pointer; }
.primary-action, .secondary-action { min-height: 36px; padding: 8px 13px; font-size: 12px; font-weight: 600; border-radius: 7px; display: inline-flex; align-items: center; justify-content: center; gap: 6px; }
.primary-action { background: var(--primary); color: var(--primary-contrast); border: 1px solid var(--primary); }
.primary-action:hover:not(:disabled) { background: var(--primary-hover); }
.secondary-action { background: var(--surface-solid); color: var(--text-main); border: 1px solid var(--border); }
button:disabled { opacity: .55; cursor: not-allowed; }
button:focus-visible, summary:focus-visible { outline: 2px solid var(--primary); outline-offset: 3px; }
.update-footer { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 11px 20px; border-top: 1px solid var(--border); background: var(--surface-alt); font-size: 11px; color: var(--text-sub); }
.text-action { border: 0; background: transparent; color: var(--primary); padding: 3px 0; white-space: nowrap; font-size: 12px; }
.text-action:hover { text-decoration: underline; }
.download-progress { padding: 0 20px 16px; }
progress { width: 100%; height: 7px; accent-color: var(--primary); border-radius: 8px; }
.progress-meta { display: flex; justify-content: space-between; color: var(--text-sub); font-size: 11px; margin-top: 6px; font-variant-numeric: tabular-nums; }
.release-notes { padding: 16px 20px; border-top: 1px solid var(--border); }
.release-notes h3 { margin: 0 0 8px; font-size: 12px; }
pre { white-space: pre-wrap; overflow-wrap: anywhere; font: inherit; font-size: 12px; line-height: 1.7; margin: 8px 0 0; max-height: 220px; overflow: auto; color: var(--text-sub); }
.update-error { margin: 0 20px 16px; padding: 12px; border-radius: 7px; background: var(--danger-soft); color: var(--danger-text); font-size: 12px; }
.update-error p { margin: 0 0 8px; line-height: 1.6; }
.update-error summary { cursor: pointer; font-size: 11px; }
.project-links { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin: 16px 0 20px; }
.project-links button { display: flex; align-items: center; justify-content: space-between; padding: 12px 14px; border: 1px solid var(--border); border-radius: 8px; color: var(--text-main); background: transparent; font-size: 12px; }
.project-links button:hover { background: var(--surface-alt); border-color: var(--primary-border); }
.project-links button span:last-child { color: var(--text-sub); }
.info-section { padding: 13px 0; border-top: 1px solid var(--border); }
.info-section h3, .info-section summary { margin: 0; font-size: 12px; font-weight: 600; }
.info-section summary { cursor: pointer; }
.info-section p { margin: 8px 0 0; font-size: 12px; color: var(--text-sub); line-height: 1.8; }
.technical-footer { display: flex; flex-wrap: wrap; gap: 6px 16px; border-top: 1px solid var(--border); padding-top: 14px; margin-top: 4px; font-size: 10px; color: var(--text-muted); }
.link-error { color: var(--danger-text); font-size: 12px; }
.spinner { width: 14px; height: 14px; fill: none; stroke: currentColor; stroke-width: 2; animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
@media (max-width: 650px) { .update-top { flex-direction: column; align-items: stretch; padding: 16px; gap: 14px; } .update-actions { align-self: flex-start; } .update-footer { padding: 10px 16px; flex-wrap: wrap; } .product-title h1 { font-size: 22px; } }
</style>
