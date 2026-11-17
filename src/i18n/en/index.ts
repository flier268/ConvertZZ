import type source from "../zh-TW";
import type { LocaleMessages } from "../types";
import common from "./common";
import app from "./app";
import lib from "./lib";
import quick from "./quick";
import clipboard from "./clipboard";
import tools from "./tools";
import about from "./about";
import dictionary from "./dictionary";
import files from "./files";
import components from "./components";
import audio from "./audio";
import settings from "./settings";

const messages: LocaleMessages<typeof source> = {
  common,
  app,
  lib,
  quick,
  clipboard,
  tools,
  about,
  dictionary,
  files,
  components,
  audio,
  settings,
};

export default messages;
