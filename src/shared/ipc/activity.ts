import { invokeCommand } from "./invoke";

export type ActivityItemDto = {
  id: string;
  type: "runtime" | "model";
  message: string;
  timestamp: string;
};

export type ActivityRecordRequest = {
  type: "runtime" | "model";
  message: string;
  id?: string;
};

export const listRecentActivity = () =>
  invokeCommand<ActivityItemDto[]>("activity_list");

export const recordRecentActivity = (request: ActivityRecordRequest) =>
  invokeCommand<ActivityItemDto>("activity_record", { request });

export const replaceRecentActivity = (items: ActivityItemDto[]) =>
  invokeCommand<ActivityItemDto[]>("activity_replace_all", { items });
