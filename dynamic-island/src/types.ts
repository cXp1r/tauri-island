export type ViewMode = "time" | "lyric" | "agent" | "search" | "sadb";

export type PrivacyUsagePayload = {

  microphone: boolean;

  camera: boolean;

};

export type WeatherResult = {

  desc: string;

  temp: number;

  city: string;

};
