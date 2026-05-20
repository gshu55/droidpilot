import { createRouter, createWebHashHistory } from "vue-router";

import MirrorPage from "../pages/MirrorPage.vue";
import SettingsPage from "../pages/SettingsPage.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: "/",
      redirect: "/mirror",
    },
    {
      path: "/commands",
      redirect: "/mirror",
    },
    {
      path: "/transfer",
      redirect: "/mirror",
    },
    {
      path: "/mirror",
      name: "mirror",
      component: MirrorPage,
    },
    {
      path: "/settings",
      name: "settings",
      component: SettingsPage,
    },
    {
      path: "/about",
      redirect: "/settings",
    },
  ],
});
