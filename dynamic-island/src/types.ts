export type ViewMode = "time" | "lyric" | "agent" | "search";

export type PrivacyUsagePayload = {

  microphone: boolean;

  camera: boolean;

};

export type WeatherResult = {

  desc: string;

  temp: number;

  city: string;

};
