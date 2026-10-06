import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api/client";
import * as pushSetup from "../../pwa/pushSetup";
import * as platform from "../../utils/platform";
import type { components } from "../../types/api";
import { NotificationsSection } from "./NotificationsSection";

type S = components["schemas"];

vi.mock("../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api/client")>();
  return {
    ...actual,
    api: {
      push: {
        listSubscriptions: vi.fn(),
        listPreferences: vi.fn(),
        sendTest: vi.fn(),
        unsubscribe: vi.fn(),
        putPreference: vi.fn(),
      },
    },
  };
});

vi.mock("../../pwa/pushSetup", () => ({
  isPushSupported: vi.fn(),
  getLocalPushState: vi.fn(),
  ensurePushSubscription: vi.fn(),
  disablePush: vi.fn(),
}));

vi.mock("../../utils/platform", () => ({
  isIOS: vi.fn(() => false),
  isStandaloneDisplay: vi.fn(() => false),
}));

const mockedApi = vi.mocked(api, { deep: true });
const mockedPushSetup = vi.mocked(pushSetup, { deep: true });
const mockedPlatform = vi.mocked(platform, { deep: true });

function baseConfig(overrides: Partial<S["ConfigResponse"]> = {}): S["ConfigResponse"] {
  return {
    userId: "u1",
    email: "sam@example.com",
    displayName: "Sam",
    vapidPublicKey: "AAEC",
    groupDefaults: {},
    memberships: [],
    ...overrides,
  };
}

function renderSection(config: S["ConfigResponse"]) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <NotificationsSection config={config} />
    </QueryClientProvider>,
  );
  return queryClient;
}

function subscription(
  overrides: Partial<S["PushSubscriptionResponse"]> = {},
): S["PushSubscriptionResponse"] {
  return {
    subscriptionId: "sub1",
    endpoint: "https://push.example/sub1",
    userAgent: "Mozilla/5.0 (Linux; Android 14; Pixel 8) Chrome/129.0",
    createdAt: "2026-01-01T00:00:00Z",
    lastSuccessAt: "2026-01-02T00:00:00Z",
    failureCount: 0,
    ...overrides,
  };
}

afterEach(() => {
  vi.clearAllMocks();
});

describe("NotificationsSection", () => {
  it("unsupported: shows the fallback copy and never calls any push API", async () => {
    mockedPushSetup.isPushSupported.mockReturnValue(false);
    mockedPlatform.isIOS.mockReturnValue(false);
    mockedApi.push.listSubscriptions.mockResolvedValue({ items: [] });
    mockedApi.push.listPreferences.mockResolvedValue({ items: [] });

    renderSection(baseConfig());

    expect(await screen.findByText(/Push isn't supported in this browser/)).toBeInTheDocument();
    expect(mockedPushSetup.getLocalPushState).not.toHaveBeenCalled();
  });

  it("iOS, not installed: shows the add-to-home-screen copy", async () => {
    mockedPushSetup.isPushSupported.mockReturnValue(true);
    mockedPlatform.isIOS.mockReturnValue(true);
    mockedPlatform.isStandaloneDisplay.mockReturnValue(false);
    mockedApi.push.listSubscriptions.mockResolvedValue({ items: [] });
    mockedApi.push.listPreferences.mockResolvedValue({ items: [] });

    renderSection(baseConfig());

    expect(await screen.findByText(/Add OpenNewsletter to your Home Screen/)).toBeInTheDocument();
  });

  it("denied permission: disables the toggle and shows the blocked copy", async () => {
    mockedPushSetup.isPushSupported.mockReturnValue(true);
    mockedPlatform.isIOS.mockReturnValue(false);
    mockedPushSetup.getLocalPushState.mockResolvedValue({ permission: "denied", endpoint: null });
    mockedApi.push.listSubscriptions.mockResolvedValue({ items: [] });
    mockedApi.push.listPreferences.mockResolvedValue({ items: [] });

    renderSection(baseConfig());

    expect(await screen.findByText(/Blocked for this browser/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Toggle push notifications" })).toBeDisabled();
  });

  it("enabled: shows the master toggle on and tags the matching device as This device", async () => {
    mockedPushSetup.isPushSupported.mockReturnValue(true);
    mockedPlatform.isIOS.mockReturnValue(false);
    mockedPushSetup.getLocalPushState.mockResolvedValue({
      permission: "granted",
      endpoint: "https://push.example/sub1",
    });
    mockedApi.push.listSubscriptions.mockResolvedValue({
      items: [
        subscription({ subscriptionId: "sub1", endpoint: "https://push.example/sub1" }),
        subscription({ subscriptionId: "sub2", endpoint: "https://push.example/sub2" }),
      ],
    });
    mockedApi.push.listPreferences.mockResolvedValue({ items: [] });

    renderSection(baseConfig());

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Toggle push notifications" })).toHaveClass("on"),
    );
    const thisDeviceBadges = await screen.findAllByText("This device");
    expect(thisDeviceBadges).toHaveLength(1);
  });

  it("send test push: renders a per-device outcome list", async () => {
    const user = userEvent.setup();
    mockedPushSetup.isPushSupported.mockReturnValue(true);
    mockedPlatform.isIOS.mockReturnValue(false);
    mockedPushSetup.getLocalPushState.mockResolvedValue({ permission: "default", endpoint: null });
    mockedApi.push.listSubscriptions.mockResolvedValue({ items: [subscription()] });
    mockedApi.push.listPreferences.mockResolvedValue({ items: [] });
    mockedApi.push.sendTest.mockResolvedValue({
      results: [
        {
          subscriptionId: "sub1",
          userAgent: subscription().userAgent,
          outcome: "delivered",
          statusCode: 201,
        },
      ],
    });

    renderSection(baseConfig());

    const sendButton = await screen.findByRole("button", { name: "Send test push" });
    await waitFor(() => expect(sendButton).toBeEnabled());
    await user.click(sendButton);

    expect(await screen.findByText(/delivered \(201\)/)).toBeInTheDocument();
  });

  it("per-group toggle: optimistically flips, then rolls back on a server error", async () => {
    const user = userEvent.setup();
    mockedPushSetup.isPushSupported.mockReturnValue(true);
    mockedPlatform.isIOS.mockReturnValue(false);
    mockedPushSetup.getLocalPushState.mockResolvedValue({ permission: "default", endpoint: null });
    mockedApi.push.listSubscriptions.mockResolvedValue({ items: [] });
    mockedApi.push.listPreferences.mockResolvedValue({
      items: [{ groupId: "g1", cycleOpen: true, deadlineReminders: true }],
    });
    let rejectPut!: (error: unknown) => void;
    mockedApi.push.putPreference.mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejectPut = reject;
        }),
    );

    renderSection(
      baseConfig({
        memberships: [
          {
            groupId: "g1",
            role: "member",
            groupName: "Group One",
            timezone: "UTC",
            gradient: "grape-sky",
          },
        ],
      }),
    );

    const cycleOpenCheckbox = await screen.findByRole("checkbox", { name: "Cycle open" });
    await waitFor(() => expect(cycleOpenCheckbox).toBeChecked());

    await user.click(cycleOpenCheckbox);
    // Optimistic: unchecked immediately.
    expect(cycleOpenCheckbox).not.toBeChecked();

    rejectPut(new Error("network error"));
    await waitFor(() => expect(cycleOpenCheckbox).toBeChecked());
  });
});
