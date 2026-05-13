<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { downloadDir } from "@tauri-apps/api/path";
import { COMMON_REMOTE_PATHS, useAdbStore } from "../stores/adb";

interface RemoteEntry {
  name: string;
  path: string;
  isDir: boolean;
}

const adbStore = useAdbStore();
const {
  transferRunning,
  uploadLocalPath,
  defaultDevicePath,
  defaultLocalDir,
  transferRecords,
  selectedSerial,
} = storeToRefs(adbStore);

const defaultDeviceRoot = "/storage/emulated/0/";
const remoteBrowserPath = ref(defaultDeviceRoot);
const remoteEntries = ref<RemoteEntry[]>([]);
const browsingRemote = ref(false);
const remoteBrowseError = ref("");
const atSdcardRoot = computed(() => remoteBrowserPath.value === defaultDeviceRoot);

function applyRemotePathToUpload(path: string) {
  adbStore.setDefaultDevicePath(path);
}

function applyRemotePathToDownload(path: string) {
  adbStore.setDefaultDevicePath(path);
}

async function pickUploadFile() {
  const selected = await open({
    directory: false,
    multiple: false,
    title: "选择要上传的本地文件",
  });

  if (typeof selected === "string") {
    adbStore.setUploadLocalPath(selected);
  }
}

async function pickDownloadDir() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择下载保存目录",
  });

  if (typeof selected === "string") {
    adbStore.setDefaultLocalDir(selected);
  }
}

async function runUpload() {
  await adbStore.uploadFile();
}

async function runQuickInstallApk() {
  await adbStore.quickInstallApk();
}

async function runDownload() {
  await adbStore.downloadFile();
}

async function loadRemoteEntries(path = remoteBrowserPath.value) {
  if (!selectedSerial.value) {
    remoteEntries.value = [];
    remoteBrowseError.value = "请先选择设备";
    return;
  }

  browsingRemote.value = true;
  remoteBrowseError.value = "";

  try {
    const normalized = path.trim() || defaultDeviceRoot;
    remoteEntries.value = await invoke<RemoteEntry[]>("list_sdcard_entries", {
      serial: selectedSerial.value,
      path: normalized,
    });
    remoteBrowserPath.value = normalized.endsWith("/") ? normalized : `${normalized}/`;
  } catch (error) {
    remoteEntries.value = [];
    remoteBrowseError.value = String(error);
  } finally {
    browsingRemote.value = false;
  }
}

function goParentDirectory() {
  if (atSdcardRoot.value) {
    return;
  }

  const trimmed = remoteBrowserPath.value.replace(/\/+$/, "");
  const idx = trimmed.lastIndexOf("/");
  const root = defaultDeviceRoot.replace(/\/+$/, "");
  let parent = idx > 0 ? trimmed.slice(0, idx) : root;
  if (!parent.startsWith(root)) {
    parent = root;
  }
  if (parent === root) {
    parent = defaultDeviceRoot;
  } else {
    parent = `${parent}/`;
  }

  void loadRemoteEntries(parent);
}

function openRemoteEntry(entry: RemoteEntry) {
  if (entry.isDir) {
    void loadRemoteEntries(entry.path);
    return;
  }

  adbStore.setDefaultDevicePath(entry.path);
}

onMounted(async () => {
  if (defaultLocalDir.value.trim()) {
    await loadRemoteEntries();
  } else {
    try {
      const dir = await downloadDir();
      if (dir && dir.trim()) {
        adbStore.setDefaultLocalDir(dir);
      }
    } catch {
      // Ignore path API errors and keep manual selection behavior.
    }
  }

  await loadRemoteEntries();
});

watch(
  () => selectedSerial.value,
  async () => {
    remoteBrowserPath.value = defaultDeviceRoot;
    await loadRemoteEntries(defaultDeviceRoot);
  },
);
</script>

<template>
  <div class="layout">
    <section class="card panel full-width">
      <h2>文件传输</h2>
      <p class="helper">上传与下载均基于当前选中设备，支持常用路径快捷填充。</p>

      <div class="transfer-grid">
        <div class="transfer-block">
          <h3>上传 Push</h3>
          <div class="field-row">
            <input
              class="input mono"
              :value="uploadLocalPath"
              placeholder="本地文件路径"
              @input="adbStore.setUploadLocalPath(($event.target as HTMLInputElement).value)"
            />
            <button type="button" class="btn btn-ghost" @click="pickUploadFile">选择文件</button>
          </div>
          <div class="field-row">
            <input
              class="input mono"
              :value="defaultDevicePath"
              placeholder="远端目标路径"
              @input="adbStore.setDefaultDevicePath(($event.target as HTMLInputElement).value)"
            />
          </div>
          <div class="path-shortcuts">
            <button
              v-for="path in COMMON_REMOTE_PATHS"
              :key="`u-${path}`"
              type="button"
              class="tiny-btn mono"
              @click="applyRemotePathToUpload(path)"
            >
              {{ path }}
            </button>
          </div>
          <button class="btn btn-primary" :disabled="transferRunning" @click="runUpload">
            {{ transferRunning ? "上传中..." : "上传文件" }}
          </button>
          <button class="btn btn-warning" :disabled="transferRunning" @click="runQuickInstallApk">
            快速安装 APK
          </button>
        </div>

        <div class="transfer-block">
          <h3>下载 Pull</h3>
          <div class="field-row">
            <input
              class="input mono"
              :value="defaultDevicePath"
              placeholder="远端源文件路径"
              @input="adbStore.setDefaultDevicePath(($event.target as HTMLInputElement).value)"
            />
          </div>
          <div class="path-shortcuts">
            <button
              v-for="path in COMMON_REMOTE_PATHS"
              :key="`d-${path}`"
              type="button"
              class="tiny-btn mono"
              @click="applyRemotePathToDownload(path)"
            >
              {{ path }}
            </button>
          </div>
          <div class="remote-browser card">
            <div class="remote-browser-head">
              <div class="mono remote-browser-path">{{ remoteBrowserPath }}</div>
              <div class="remote-browser-actions">
                <button type="button" class="tiny-btn" :disabled="atSdcardRoot || browsingRemote" @click="goParentDirectory">上级</button>
                <button type="button" class="tiny-btn" :disabled="browsingRemote" @click="loadRemoteEntries()">刷新</button>
              </div>
            </div>

            <p class="helper" v-if="!selectedSerial">请先选择设备后浏览目录</p>
            <p class="helper" v-else-if="browsingRemote">目录读取中...</p>
            <p class="helper" v-else-if="remoteBrowseError">{{ remoteBrowseError }}</p>

            <div class="remote-list" v-else>
              <button
                v-for="entry in remoteEntries"
                :key="entry.path"
                type="button"
                class="remote-item"
                :class="{ active: !entry.isDir && entry.path === defaultDevicePath }"
                @click="openRemoteEntry(entry)"
              >
                <span class="remote-item-name mono">{{ entry.isDir ? `[目录] ${entry.name}` : entry.name }}</span>
                <span class="remote-item-type">{{ entry.isDir ? "进入" : "选择" }}</span>
              </button>
              <p class="helper" v-if="remoteEntries.length === 0">当前目录为空</p>
            </div>
          </div>
          <div class="field-row">
            <input
              class="input mono"
              :value="defaultLocalDir"
              placeholder="本地保存目录"
              @input="adbStore.setDefaultLocalDir(($event.target as HTMLInputElement).value)"
            />
            <button type="button" class="btn btn-ghost" @click="pickDownloadDir">选择目录</button>
          </div>
          <button class="btn btn-primary" :disabled="transferRunning" @click="runDownload">
            {{ transferRunning ? "下载中..." : "下载文件" }}
          </button>
        </div>
      </div>
    </section>

    <section class="card panel full-width">
      <div class="record-headline">
        <h2>传输记录</h2>
        <button type="button" class="btn btn-danger" @click="adbStore.clearTransferRecords">清空记录</button>
      </div>

      <p class="helper" v-if="transferRecords.length === 0">暂无传输记录，上传或下载后会显示在这里。</p>
      <div v-else class="record-list">
        <article v-for="record in transferRecords" :key="record.id" class="record-item">
          <header class="record-head">
            <div class="mono command-line">{{ record.fullCommand }}</div>
            <span class="run-badge" :class="record.status">
              {{ record.status === "success" ? "成功" : "失败" }}
            </span>
          </header>
          <div class="meta-row">
            <span>方向：{{ record.direction }}</span>
            <span>时间：{{ record.executedAt }}</span>
            <span>设备：{{ record.deviceSerial }}</span>
            <span>退出码：{{ record.statusCode }}</span>
          </div>
          <div class="meta-row mono">
            <span>本地：{{ record.localPath }}</span>
          </div>
          <div class="meta-row mono">
            <span>远端：{{ record.remotePath }}</span>
          </div>
          <pre v-if="record.stdout" class="out-block"><strong>stdout</strong>
{{ record.stdout }}</pre>
          <pre v-if="record.stderr" class="err-block"><strong>stderr</strong>
{{ record.stderr }}</pre>
        </article>
      </div>
    </section>
  </div>
</template>
