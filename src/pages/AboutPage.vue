<script setup lang="ts">
import { getTauriVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { onMounted, ref } from "vue";
import { useI18n } from "../i18n";

const appVersion = ref(__APP_VERSION__);
const { t } = useI18n();
const tauriVersion = ref("");

onMounted(async () => {
  try {
    const version = await invoke<string>("get_cargo_package_version");

    if (version.trim()) {
      appVersion.value = version;
    }
  } catch {
    appVersion.value = __APP_VERSION__;
  }

  try {
    const version = await getTauriVersion();

    if (version.trim()) {
      tauriVersion.value = version;
    }
  } catch {
    tauriVersion.value = "";
  }
});
</script>

<template>
  <section class="card panel about-page">
    <header class="about-head">
      <h4>{{ t("common.appName") }}</h4>
      <p>{{ t("about.subtitle") }}</p>
    </header>

    <div class="about-grid">
      <article class="about-item">
        <h3>{{ t("about.versionInfo") }}</h3>
        <ul>
          <li>{{ t("about.appVersion") }}：{{ appVersion }}</li>
          <li>{{ t("about.tauriVersion") }}：{{ tauriVersion || t("common.unavailable") }}</li>
          <li>{{ t("about.runtime") }}：{{ t("about.runtimeLocal") }}</li>
        </ul>
      </article>

      <article class="about-item">
        <h3>{{ t("about.statement") }}</h3>
        <ul>
          <li>{{ t("about.statement1") }}</li>
          <li>{{ t("about.statement2") }}</li>
          <li>{{ t("about.statement3") }}</li>
        </ul>
      </article>

      <article class="about-item">
        <h3>{{ t("about.privacy") }}</h3>
        <ul>
          <li>{{ t("about.privacy1") }}</li>
          <li>{{ t("about.privacy2") }}</li>
          <li>{{ t("about.privacy3") }}</li>
          <li>{{ t("about.privacy4") }}</li>
        </ul>
      </article>

      <article class="about-item">
        <h3>{{ t("about.oss") }}</h3>
        <p>{{ t("about.ossText") }}</p>
      </article>
    </div>
  </section>
</template>

<style scoped>
.about-page {
  margin: 0 auto;
}

.about-head {
  margin-bottom: 12px;
}

.about-head h4 {
  margin: 0 0 8px;
  color: var(--text-main);
  font-size: 16px;
  font-weight: 600;
}

.about-head p {
  margin: 0;
  color: var(--text-sub);
  font-size: 12px;
  line-height: 1.45;
}

.about-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 8px;
}

.about-item {
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 10px;
  background: var(--surface-alt);
}

.about-item:first-child {
  border-top: 1px solid var(--border);
}

.about-item h3 {
  margin: 0 0 8px;
  font-size: 14px;
  line-height: 1.35;
}

.about-item ul {
  margin: 0;
  padding-left: 18px;
  color: var(--control-text);
  font-size: 13px;
  line-height: 1.55;
}

.about-item p {
  margin: 0;
  color: var(--text-sub);
  font-size: 13px;
  line-height: 1.6;
}
</style>
