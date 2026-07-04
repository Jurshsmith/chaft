#include <QAbstractSocket>
#include <QByteArray>
#include <QClipboard>
#include <QCoreApplication>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QHostAddress>
#include <QIODevice>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QLibrary>
#include <QMetaObject>
#include <QObject>
#include <QPointer>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QPixmap>
#include <QSaveFile>
#include <QScreen>
#include <QStandardPaths>
#include <QStringList>
#include <QThread>
#include <QTimer>
#include <QUrl>
#include <QVariant>
#include <QVariantList>
#include <QVariantMap>
#include <QWindow>
#include <QtGlobal>
#include <algorithm>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <memory>
#include <utility>

namespace {

using StoreSnapshotResultJsonFn = char *(*)(const char *, const char *);
using StoreSnapshotLatestResultJsonFn = char *(*)(const char *, const char *,
                                                  std::size_t);
using StoreSnapshotWindowResultJsonFn = char *(*)(const char *, const char *,
                                                  std::size_t, std::size_t);
using RuntimeSnapshotResultJsonFn = char *(*)(const char *, const char *,
                                              const char *);
using RuntimeSnapshotLatestResultJsonFn = char *(*)(const char *, const char *,
                                                    const char *, std::size_t);
using RuntimeSnapshotWindowResultJsonFn = char *(*)(const char *, const char *,
                                                    const char *, std::size_t,
                                                    std::size_t);
using RuntimeChannelSnapshotLatestResultJsonFn = char *(*)(const char *,
                                                           const char *,
                                                           const char *,
                                                           const char *,
                                                           std::size_t);
using RuntimeChannelSnapshotWindowResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              std::size_t, std::size_t);
using RuntimeDeviceIdResultJsonFn = char *(*)(const char *, const char *);
using RuntimeListWorkspacesResultJsonFn = char *(*)(const char *, const char *);
using RuntimeListWorkspacePageResultJsonFn = char *(*)(const char *,
                                                       const char *,
                                                       std::size_t,
                                                       std::size_t);
using RuntimeListWorkspaceChannelPageResultJsonFn = char *(*)(const char *,
                                                              const char *,
                                                              const char *,
                                                              std::size_t,
                                                              std::size_t);
using RuntimeListWorkspaceChannelPageContainingResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              std::size_t);
using RuntimeListWorkspaceMemberPageResultJsonFn = char *(*)(const char *,
                                                             const char *,
                                                             const char *,
                                                             std::size_t,
                                                             std::size_t);
using RuntimeCreateWorkspaceResultJsonFn = char *(*)(const char *, const char *,
                                                     const char *,
                                                     const char *);
using RuntimeCreateChannelResultJsonFn = char *(*)(const char *, const char *,
                                                   const char *, const char *,
                                                   bool);
using RuntimeUpdateDeviceProfileResultJsonFn = char *(*)(const char *,
                                                         const char *,
                                                         const char *,
                                                         const char *);
using RuntimePublishDeviceKeyPackageResultJsonFn = char *(*)(const char *,
                                                             const char *,
                                                             const char *,
                                                             const char *,
                                                             const char *);
using RuntimePublishPeerEndpointResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *, const char *, bool, bool, qint64);
using RuntimePublishPeerEndpointWithReplicaCapabilityResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *, const char *, bool, bool, qint64, const char *,
              const char *);
using RuntimeOpenMlsWorkspaceActionResultJsonFn = char *(*)(const char *,
                                                            const char *,
                                                            const char *);
using RuntimeOpenMlsWorkspaceValueResultJsonFn = char *(*)(const char *,
                                                           const char *,
                                                           const char *,
                                                           const char *);
using RuntimeOpenMlsChannelActionResultJsonFn = char *(*)(const char *,
                                                          const char *,
                                                          const char *,
                                                          const char *);
using RuntimeOpenMlsChannelValueResultJsonFn = char *(*)(const char *,
                                                         const char *,
                                                         const char *,
                                                         const char *,
                                                         const char *);
using RuntimeSendMessageResultJsonFn = char *(*)(const char *, const char *,
                                                 const char *, const char *,
                                                 const char *);
using RuntimeSendMessageReplyResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *, const char *);
using RuntimeSendAttachmentResultJsonFn = char *(*)(const char *, const char *,
                                                    const char *, const char *,
                                                    const char *, const char *,
                                                    const char *);
using RuntimeSendAttachmentReplyResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *, const char *, const char *, const char *);
using RuntimeSaveAttachmentResultJsonFn = char *(*)(const char *, const char *,
                                                    const char *, const char *,
                                                    const char *, const char *);
using RuntimePruneBlobsResultJsonFn = char *(*)(const char *, const char *);
using RuntimeEditMessageResultJsonFn = char *(*)(const char *, const char *,
                                                 const char *, const char *,
                                                 const char *);
using RuntimeDeleteMessageResultJsonFn = char *(*)(const char *, const char *,
                                                   const char *, const char *);
using RuntimeAddReactionResultJsonFn = char *(*)(const char *, const char *,
                                                 const char *, const char *,
                                                 const char *);
using RuntimeRemoveReactionResultJsonFn = char *(*)(const char *, const char *,
                                                    const char *, const char *,
                                                    const char *);
using RuntimeMarkChannelReadResultJsonFn = char *(*)(const char *, const char *,
                                                     const char *,
                                                     const char *);
using RuntimeInviteMemberResultJsonFn = char *(*)(const char *, const char *,
                                                  const char *, const char *,
                                                  const char *);
using RuntimeRemoveMemberResultJsonFn = char *(*)(const char *, const char *,
                                                  const char *, const char *);
using RuntimeAddChannelMemberResultJsonFn = char *(*)(const char *,
                                                      const char *,
                                                      const char *,
                                                      const char *,
                                                      const char *);
using RuntimeRemoveChannelMemberResultJsonFn = char *(*)(const char *,
                                                         const char *,
                                                         const char *,
                                                         const char *,
                                                         const char *);
using RuntimeExportWorkspaceKeyResultJsonFn = char *(*)(const char *,
                                                        const char *,
                                                        const char *);
using RuntimeExportTrustSnapshotResultJsonFn = char *(*)(const char *,
                                                         const char *,
                                                         const char *);
using RuntimeRotateWorkspaceManualKeysResultJsonFn = char *(*)(const char *,
                                                               const char *,
                                                               const char *);
using RuntimeRotateWorkspaceForSuspectedCompromiseResultJsonFn =
    char *(*)(const char *, const char *, const char *);
using RuntimeDetectCompromiseResultJsonFn = char *(*)(const char *,
                                                      const char *,
                                                      const char *);
using RuntimeImportWorkspaceKeyResultJsonFn = char *(*)(const char *,
                                                        const char *,
                                                        const char *);
using RuntimeExportChannelKeyResultJsonFn = char *(*)(const char *,
                                                      const char *,
                                                      const char *,
                                                      const char *);
using RuntimeRotateChannelKeyResultJsonFn = char *(*)(const char *,
                                                      const char *,
                                                      const char *,
                                                      const char *);
using RuntimeImportChannelKeyResultJsonFn = char *(*)(const char *,
                                                      const char *,
                                                      const char *);
using RuntimeExportRecoveryBundleResultJsonFn = char *(*)(const char *,
                                                          const char *,
                                                          const char *,
                                                          const char *);
using RuntimeImportRecoveryBundleResultJsonFn = char *(*)(const char *,
                                                          const char *,
                                                          const char *,
                                                          const char *);
using RuntimeReindexWorkspaceSearchResultJsonFn = char *(*)(const char *,
                                                            const char *,
                                                            const char *);
using RuntimeSearchWorkspaceResultJsonFn = char *(*)(const char *, const char *,
                                                     const char *,
                                                     const char *);
using RuntimeSearchWorkspaceChannelsResultJsonFn = char *(*)(const char *,
                                                             const char *,
                                                             const char *,
                                                             const char *,
                                                             std::size_t);
using RuntimeDirectSyncResultJsonFn = char *(*)(const char *, const char *,
                                                const char *, const char *);
using RuntimeDirectRetryResultJsonFn = char *(*)(const char *, const char *,
                                                 const char *, const char *);
using RuntimeDirectEventPublishResultJsonFn = char *(*)(const char *,
                                                        const char *,
                                                        const char *,
                                                        const char *,
                                                        const char *);
using RuntimeWorkspacePublishQueueResultJsonFn = char *(*)(const char *,
                                                           const char *,
                                                           const char *);
using RuntimeWorkspaceStorageHealthResultJsonFn = char *(*)(const char *,
                                                            const char *,
                                                            const char *);
using RuntimeRepairWorkspaceStorageMetadataResultJsonFn =
    char *(*)(const char *, const char *, const char *);
using RuntimeStartDirectPeerResultJsonFn = char *(*)(const char *, const char *,
                                                     const char *);
using RuntimeStartIrohPeerResultJsonFn = char *(*)(const char *, const char *);
using RuntimeStopDirectPeerResultJsonFn = char *(*)(const char *);
using RuntimeSetIdentityPassphraseFn = bool (*)(const char *, const char *);
using RuntimeClearIdentityPassphraseFn = bool (*)(const char *);
using FreeStringFn = void (*)(char *);

enum class DirectSyncMode { Publish, Backup, Pull, Sync };

constexpr qsizetype kMaxDesktopFfiJsonBytes = 16 * 1024 * 1024;

QVariantMap snapshotFromJson(const QByteArray &json) {
  const auto document = QJsonDocument::fromJson(json);
  if (!document.isObject()) {
    return {};
  }

  return document.object().toVariantMap();
}

QJsonObject resultValueFromJson(const QByteArray &json, QString *errorMessage) {
  const auto document = QJsonDocument::fromJson(json);
  if (!document.isObject()) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("invalid FFI JSON result");
    }
    return {};
  }

  const auto result = document.object();
  if (!result.value(QStringLiteral("ok")).toBool()) {
    const auto error = result.value(QStringLiteral("error")).toObject();
    const auto code = error.value(QStringLiteral("code"))
                          .toString(QStringLiteral("ffi_error"));
    const auto message = error.value(QStringLiteral("message"))
                             .toString(QStringLiteral("unknown error"));
    if (errorMessage != nullptr) {
      *errorMessage = code + QStringLiteral(": ") + message;
    }
    return {};
  }

  const auto value = result.value(QStringLiteral("value"));
  if (!value.isObject()) {
    if (errorMessage != nullptr) {
      *errorMessage =
          QStringLiteral("FFI result did not contain an object value");
    }
    return {};
  }

  return value.toObject();
}

QString resultErrorCodeFromJson(const QByteArray &json) {
  const auto document = QJsonDocument::fromJson(json);
  if (!document.isObject()) {
    return QStringLiteral("ffi_error");
  }

  const auto result = document.object();
  if (result.value(QStringLiteral("ok")).toBool()) {
    return {};
  }
  return result.value(QStringLiteral("error"))
      .toObject()
      .value(QStringLiteral("code"))
      .toString(QStringLiteral("ffi_error"));
}

bool isPeerProtocolFailureCode(const QString &code) {
  return code == QStringLiteral("runtime_peer_protocol_failed");
}

int jsonCountOrArraySize(const QJsonObject &object, const QString &countField,
                         const QString &arrayField) {
  const auto countValue = object.value(countField);
  if (countValue.isDouble()) {
    return std::max(0, countValue.toInt(0));
  }

  return object.value(arrayField).toArray().size();
}

int openMlsCatchupEventCountFromJson(const QJsonObject &openMlsCatchup) {
  const auto countValue = openMlsCatchup.value(QStringLiteral("eventCount"));
  if (countValue.isDouble()) {
    return std::max(0, countValue.toInt(0));
  }

  auto eventCount =
      openMlsCatchup.value(QStringLiteral("workspaceAppliedEventIds"))
          .toArray()
          .size();
  eventCount +=
      openMlsCatchup.value(QStringLiteral("workspaceProvisionedEventIds"))
          .toArray()
          .size();
  if (!openMlsCatchup.value(QStringLiteral("workspaceJoinedEventId"))
           .toString()
           .isEmpty()) {
    eventCount += 1;
  }
  for (const auto &channelGroup :
       openMlsCatchup.value(QStringLiteral("channelGroups")).toArray()) {
    const auto channelGroupObject = channelGroup.toObject();
    const auto groupCount =
        channelGroupObject.value(QStringLiteral("eventCount"));
    if (groupCount.isDouble()) {
      eventCount += std::max(0, groupCount.toInt(0));
      continue;
    }
    eventCount += channelGroupObject.value(QStringLiteral("appliedEventIds"))
                      .toArray()
                      .size();
    eventCount +=
        channelGroupObject.value(QStringLiteral("provisionedEventIds"))
            .toArray()
            .size();
    if (!channelGroupObject.value(QStringLiteral("joinedEventId"))
             .toString()
             .isEmpty()) {
      eventCount += 1;
    }
  }

  return eventCount;
}

QString compromiseSkippedReasonLabel(const QString &reason) {
  if (reason == QStringLiteral("remote_signals_require_review")) {
    return QStringLiteral("remote review required");
  }
  if (reason == QStringLiteral("local_signals_already_handled")) {
    return QStringLiteral("already handled");
  }
  if (reason == QStringLiteral("local_secret_state_missing")) {
    return QStringLiteral("local secret state missing");
  }
  if (reason == QStringLiteral("no_signals")) {
    return QStringLiteral("no signals");
  }

  return reason;
}

QString compromiseResponseSummaryText(const QJsonValue &responseValue) {
  if (!responseValue.isObject()) {
    return {};
  }

  const auto response = responseValue.toObject();
  const auto report = response.value(QStringLiteral("report")).toObject();
  const auto signalCount = report.value(QStringLiteral("signalCount")).toInt(0);
  if (signalCount <= 0) {
    return {};
  }

  const auto rotation = response.value(QStringLiteral("rotation")).toObject();
  const auto rotationEventCount =
      jsonCountOrArraySize(rotation, QStringLiteral("rotatedEventCount"),
                           QStringLiteral("rotatedEventIds"));
  if (rotationEventCount > 0 ||
      response.value(QStringLiteral("rotatedLocalSecretState")).toBool(false)) {
    return QStringLiteral("security rotated %1 event(s) for %2 signal(s)")
        .arg(rotationEventCount)
        .arg(signalCount);
  }

  const auto alreadyHandledCount = jsonCountOrArraySize(
      response, QStringLiteral("alreadyHandledSignalCount"),
      QStringLiteral("alreadyHandledSignalEventIds"));
  const auto respondedCount =
      jsonCountOrArraySize(response, QStringLiteral("respondedSignalCount"),
                           QStringLiteral("respondedSignalEventIds"));
  if (alreadyHandledCount > 0 && respondedCount == 0) {
    return QStringLiteral("security already handled %1 signal(s)")
        .arg(alreadyHandledCount);
  }

  const auto skippedReason = compromiseSkippedReasonLabel(
      response.value(QStringLiteral("skippedReason")).toString());
  if (!skippedReason.isEmpty()) {
    return QStringLiteral("security review %1 signal(s), %2")
        .arg(signalCount)
        .arg(skippedReason);
  }

  return QStringLiteral("security reviewed %1 signal(s)").arg(signalCount);
}

QString compromiseReportSummaryText(const QJsonObject &report) {
  const auto signalCount = report.value(QStringLiteral("signalCount")).toInt(0);
  if (signalCount <= 0) {
    return QStringLiteral("security review clean");
  }

  QStringList parts;
  parts << QStringLiteral("%1 signal(s)").arg(signalCount);

  const auto localDeviceSignalCount =
      report.value(QStringLiteral("localDeviceSignalCount")).toInt(0);
  if (localDeviceSignalCount > 0) {
    parts << QStringLiteral("%1 local").arg(localDeviceSignalCount);
  }

  const auto invalidSignatureCount =
      report.value(QStringLiteral("invalidSignatureCount")).toInt(0);
  if (invalidSignatureCount > 0) {
    parts
        << QStringLiteral("%1 failed signature(s)").arg(invalidSignatureCount);
  }

  if (report.value(QStringLiteral("shouldRotateLocalSecretState"))
          .toBool(false)) {
    parts << QStringLiteral("rotation recommended");
  } else {
    const auto recommendedAction =
        report.value(QStringLiteral("recommendedAction")).toString();
    if (!recommendedAction.isEmpty()) {
      parts << recommendedAction;
    }
  }

  return QStringLiteral("security review: %1").arg(parts.join(" | "));
}

QString reindexSearchSummaryText(const QJsonObject &report) {
  const auto indexedMessageCount =
      report.value(QStringLiteral("indexedMessageCount")).toInt(0);
  return QStringLiteral("search reindexed %1 message(s)")
      .arg(indexedMessageCount);
}

QByteArray takeFfiString(char *raw, FreeStringFn freeString,
                         QString *errorMessage = nullptr);

QJsonObject latestRuntimeSnapshotValue(
    RuntimeSnapshotResultJsonFn snapshotFn,
    RuntimeSnapshotLatestResultJsonFn snapshotLatestFn, FreeStringFn freeString,
    const QByteArray &runtimeDirBytes, const QByteArray &identityFileBytes,
    const QByteArray &workspaceIdBytes, std::size_t timelineLimit,
    QString *errorMessage) {
  if (freeString == nullptr ||
      (snapshotFn == nullptr && snapshotLatestFn == nullptr)) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("runtime snapshot unavailable");
    }
    return {};
  }

  char *raw =
      snapshotLatestFn != nullptr
          ? snapshotLatestFn(runtimeDirBytes.constData(),
                             identityFileBytes.isEmpty()
                                 ? nullptr
                                 : identityFileBytes.constData(),
                             workspaceIdBytes.constData(), timelineLimit)
          : snapshotFn(runtimeDirBytes.constData(),
                       identityFileBytes.isEmpty()
                           ? nullptr
                           : identityFileBytes.constData(),
                       workspaceIdBytes.constData());

  QString readError;
  const auto json = takeFfiString(raw, freeString, &readError);
  if (!readError.isEmpty()) {
    if (errorMessage != nullptr) {
      *errorMessage = readError;
    }
    return {};
  }
  if (json.isEmpty()) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("empty FFI result");
    }
    return {};
  }

  return resultValueFromJson(json, errorMessage);
}

QVariantList resultArrayValueFromJson(const QByteArray &json,
                                      QString *errorMessage) {
  const auto document = QJsonDocument::fromJson(json);
  if (!document.isObject()) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("invalid FFI JSON result");
    }
    return {};
  }

  const auto result = document.object();
  if (!result.value(QStringLiteral("ok")).toBool()) {
    const auto error = result.value(QStringLiteral("error")).toObject();
    const auto code = error.value(QStringLiteral("code"))
                          .toString(QStringLiteral("ffi_error"));
    const auto message = error.value(QStringLiteral("message"))
                             .toString(QStringLiteral("unknown error"));
    if (errorMessage != nullptr) {
      *errorMessage = code + QStringLiteral(": ") + message;
    }
    return {};
  }

  const auto value = result.value(QStringLiteral("value"));
  if (!value.isArray()) {
    if (errorMessage != nullptr) {
      *errorMessage =
          QStringLiteral("FFI result did not contain an array value");
    }
    return {};
  }

  return value.toArray().toVariantList();
}

QByteArray takeBoundedFfiString(char *raw, FreeStringFn freeString,
                                qsizetype maxBytes, const QString &label,
                                QString *errorMessage) {
  QByteArray json;
  if (raw == nullptr) {
    return json;
  }

  qsizetype length = 0;
  while (length <= maxBytes && raw[length] != '\0') {
    ++length;
  }
  if (length > maxBytes) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("%1 FFI JSON exceeded %2 bytes")
                          .arg(label)
                          .arg(maxBytes);
    }
    freeString(raw);
    return {};
  }

  json = QByteArray(raw, length);
  freeString(raw);
  return json;
}

QByteArray takeFfiString(char *raw, FreeStringFn freeString,
                         QString *errorMessage) {
  return takeBoundedFfiString(raw, freeString, kMaxDesktopFfiJsonBytes,
                              QStringLiteral("desktop FFI result"),
                              errorMessage);
}

QVariantList workspaceSummariesFromRuntime(
    RuntimeListWorkspacesResultJsonFn listFn,
    RuntimeListWorkspacePageResultJsonFn listPageFn, FreeStringFn freeString,
    const QByteArray &runtimeDirBytes, const QByteArray &identityFileBytes,
    QString *errorMessage) {
  if (freeString == nullptr) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("workspace summary FFI unavailable");
    }
    return {};
  }

  const auto identityFile =
      identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData();
  constexpr qsizetype workspaceSummaryPageLimit = 128;
  constexpr qsizetype workspaceSummaryJsonMaxBytes = 512 * 1024;
  if (listPageFn == nullptr) {
    if (listFn == nullptr) {
      if (errorMessage != nullptr) {
        *errorMessage = QStringLiteral("workspace summary FFI unavailable");
      }
      return {};
    }
    QString readError;
    const auto json =
        takeBoundedFfiString(listFn(runtimeDirBytes.constData(), identityFile),
                             freeString, workspaceSummaryJsonMaxBytes,
                             QStringLiteral("workspace summary"), &readError);
    if (!readError.isEmpty()) {
      if (errorMessage != nullptr) {
        *errorMessage = readError;
      }
      return {};
    }
    auto summaries = resultArrayValueFromJson(json, errorMessage);
    while (summaries.size() > workspaceSummaryPageLimit) {
      summaries.removeLast();
    }
    return summaries;
  }

  QString pageError;
  const auto json = takeBoundedFfiString(
      listPageFn(runtimeDirBytes.constData(), identityFile, 0,
                 static_cast<std::size_t>(workspaceSummaryPageLimit)),
      freeString, workspaceSummaryJsonMaxBytes,
      QStringLiteral("workspace summary page"), &pageError);
  if (!pageError.isEmpty()) {
    if (errorMessage != nullptr) {
      *errorMessage = pageError;
    }
    return {};
  }
  const auto page = resultValueFromJson(json, &pageError);
  if (!pageError.isEmpty()) {
    if (errorMessage != nullptr) {
      *errorMessage = pageError;
    }
    return {};
  }

  const auto workspacesValue = page.value(QStringLiteral("workspaces"));
  if (!workspacesValue.isArray()) {
    if (errorMessage != nullptr) {
      *errorMessage =
          QStringLiteral("workspace page did not contain workspace rows");
    }
    return {};
  }
  return workspacesValue.toArray().toVariantList();
}

QByteArray fallbackSnapshotJson() {
  return R"json({
        "workspaceId": "wrk_demo",
        "name": "Chaft Labs",
        "channels": [
            { "channelId": "chn_general", "name": "general", "isPrivate": false, "unreadCount": 0 },
            { "channelId": "chn_runtime", "name": "p2p-runtime", "isPrivate": false, "unreadCount": 2 },
            { "channelId": "chn_design", "name": "design-system", "isPrivate": false, "unreadCount": 0 },
            { "channelId": "chn_replicas", "name": "replica-nodes", "isPrivate": true, "unreadCount": 1 }
        ],
        "profiles": [
            { "deviceId": "dev_mira", "displayName": "Mira", "updatedEventId": "evt_profile_mira" }
        ],
        "members": [
            {
                "deviceId": "dev_mira",
                "role": "owner",
                "displayName": "Mira",
                "profileEventId": "evt_profile_mira",
                "membershipEventId": "evt_workspace"
            }
        ],
        "keyPackages": [
            {
                "deviceId": "dev_mira",
                "keyPackageId": "dkp_mira_demo",
                "protocol": "openmls/key-package",
                "byteLen": 512,
                "publishedEventId": "evt_key_package_mira",
                "physicalMs": 1700000000010
            }
        ],
        "peerEndpoints": [],
        "timeline": [
            {
                "kind": "encrypted_message",
                "eventId": "evt_ciphertext",
                "messageId": "msg_ciphertext",
                "channelId": "chn_general",
                "authorDeviceId": "dev_mira",
                "authorDisplayName": "Mira",
                "body": "Encrypted message",
                "attachmentCount": 0,
                "attachments": [],
                "reactionCount": 0,
                "reactions": {},
                "myReactions": [],
                "encrypted": true,
                "deleted": false,
                "missingParentIds": []
            },
            {
                "kind": "missing_history_gap",
                "eventId": "evt_later_slice",
                "messageId": null,
                "channelId": null,
                "authorDeviceId": null,
                "authorDisplayName": null,
                "body": "Missing 2 parent event(s)",
                "attachmentCount": 0,
                "attachments": [],
                "reactionCount": 0,
                "reactions": {},
                "myReactions": [],
                "encrypted": false,
                "deleted": false,
                "missingParentIds": ["evt_parent_a", "evt_parent_b"]
            }
        ],
        "gapCount": 0,
        "gaps": [],
        "invalidSignatureCount": 0,
        "invalidSignatures": []
    })json";
}

QString ffiLibraryFileName() {
#if defined(Q_OS_WIN)
  return QStringLiteral("chaft_ffi.dll");
#elif defined(Q_OS_MACOS)
  return QStringLiteral("libchaft_ffi.dylib");
#else
  return QStringLiteral("libchaft_ffi.so");
#endif
}

constexpr qsizetype kMaxDesktopPathBytes = 64 * 1024;
constexpr qsizetype kMaxDesktopPassphraseBytes = 16 * 1024;

bool desktopPathWithinLimit(const QString &path) {
  return !path.isEmpty() && path.toUtf8().size() <= kMaxDesktopPathBytes;
}

QString normalizedDesktopPath(QString path) {
  return desktopPathWithinLimit(path) ? path : QString();
}

QString normalizedEnvironmentPath(QString path) {
  if (path.trimmed().isEmpty()) {
    return {};
  }
  return normalizedDesktopPath(path);
}

QString normalizedEnvironmentPassphrase(QString passphrase) {
  if (passphrase.trimmed().isEmpty() ||
      passphrase.toUtf8().size() > kMaxDesktopPassphraseBytes) {
    return {};
  }
  return passphrase;
}

void appendDesktopPathCandidate(QStringList *candidates, const QString &path) {
  if (desktopPathWithinLimit(path)) {
    candidates->append(path);
  }
}

QStringList ffiLibraryCandidates() {
  QStringList candidates;
  const auto envPath = qEnvironmentVariable("CHAFT_FFI_LIBRARY");
  appendDesktopPathCandidate(&candidates, normalizedEnvironmentPath(envPath));

  const auto libraryName = ffiLibraryFileName();
  const auto appDir = QCoreApplication::applicationDirPath();
  const auto currentDir = QDir::currentPath();

  appendDesktopPathCandidate(&candidates, QDir(appDir).filePath(libraryName));
  appendDesktopPathCandidate(
      &candidates,
      QDir(appDir).filePath(QStringLiteral("../lib/") + libraryName));
  appendDesktopPathCandidate(
      &candidates,
      QDir(currentDir).filePath(QStringLiteral("target/debug/") + libraryName));
  appendDesktopPathCandidate(
      &candidates,
      QDir(currentDir).filePath(QStringLiteral("target/release/") + libraryName));
  appendDesktopPathCandidate(
      &candidates,
      QDir(currentDir)
          .filePath(QStringLiteral("../../target/debug/") + libraryName));
  appendDesktopPathCandidate(
      &candidates,
      QDir(currentDir)
          .filePath(QStringLiteral("../../target/release/") + libraryName));
  return candidates;
}

void addDesktopQmlImportPath(QQmlApplicationEngine *engine, QStringList *added,
                             const QString &path) {
  const QDir dir(path);
  if (!dir.exists()) {
    return;
  }

  auto normalized = dir.canonicalPath();
  if (normalized.isEmpty()) {
    normalized = dir.absolutePath();
  }
  if (added->contains(normalized)) {
    return;
  }

  engine->addImportPath(normalized);
  added->append(normalized);
}

void addDesktopQmlImportPaths(QQmlApplicationEngine *engine) {
  QStringList added;
  const auto appDir = QDir(QCoreApplication::applicationDirPath());
  const auto currentDir = QDir::current();
  const auto envImportRoot =
      normalizedEnvironmentPath(qEnvironmentVariable("CHAFT_DESKTOP_QML_IMPORT_ROOT"));

  addDesktopQmlImportPath(engine, &added, envImportRoot);
  if (!envImportRoot.isEmpty()) {
    return;
  }
  addDesktopQmlImportPath(engine, &added, appDir.absolutePath());
  addDesktopQmlImportPath(engine, &added, appDir.absoluteFilePath("../../.."));
  addDesktopQmlImportPath(engine, &added,
                          appDir.absoluteFilePath("../Resources/qml"));

  for (const auto &preset :
       {QStringLiteral("desktop-debug"), QStringLiteral("desktop-release")}) {
    const auto buildModuleRoot =
        currentDir.absoluteFilePath(QStringLiteral("build/%1/apps/desktop-qt")
                                        .arg(preset));
    addDesktopQmlImportPath(engine, &added, buildModuleRoot);
    addDesktopQmlImportPath(engine, &added,
                            QDir(buildModuleRoot).filePath("Chaft/qml"));
  }

  addDesktopQmlImportPath(engine, &added,
                          currentDir.absoluteFilePath("apps/desktop-qt/qml"));
}

void loadDesktopQml(QQmlApplicationEngine *engine) {
  const auto configuredFile =
      normalizedEnvironmentPath(qEnvironmentVariable("CHAFT_DESKTOP_QML_FILE"));
  const QFileInfo configuredInfo(configuredFile);
  if (!configuredFile.isEmpty() && configuredInfo.exists() &&
      configuredInfo.isFile()) {
    engine->load(QUrl::fromLocalFile(configuredInfo.absoluteFilePath()));
    return;
  }

  engine->loadFromModule("Chaft", "App");
}

[[noreturn]] void finishDesktopSmoke(int code) {
  std::fflush(stdout);
  std::fflush(stderr);
  std::_Exit(code);
}

QString defaultRuntimeDir() {
  const auto configuredRuntimeDir =
      normalizedEnvironmentPath(qEnvironmentVariable("CHAFT_RUNTIME_DIR"));
  if (!configuredRuntimeDir.isEmpty()) {
    return configuredRuntimeDir;
  }

  const auto appDataDir =
      QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
  if (desktopPathWithinLimit(appDataDir)) {
    return appDataDir;
  }

  const auto fallbackDir =
      QDir(QDir::homePath()).filePath(QStringLiteral(".chaft"));
  return desktopPathWithinLimit(fallbackDir) ? fallbackDir : QString();
}

QString desktopConfigPath(const QString &runtimeDir) {
  if (!desktopPathWithinLimit(runtimeDir)) {
    return {};
  }
  const auto configPath =
      QDir(runtimeDir).filePath(QStringLiteral("desktop.json"));
  return desktopPathWithinLimit(configPath) ? configPath : QString();
}

constexpr qint64 kMaxDesktopConfigBytes = 64LL * 1024;
constexpr qsizetype kMaxWorkspaceIdBytes = 128;

QJsonObject loadDesktopConfig(const QString &runtimeDir) {
  const auto configPath = desktopConfigPath(runtimeDir);
  if (configPath.isEmpty()) {
    return {};
  }
  QFile file(configPath);
  if (!file.open(QIODevice::ReadOnly)) {
    return {};
  }
  if (file.size() > kMaxDesktopConfigBytes) {
    return {};
  }

  const auto bytes = file.read(kMaxDesktopConfigBytes + 1);
  if (bytes.size() > kMaxDesktopConfigBytes) {
    return {};
  }
  const auto document = QJsonDocument::fromJson(bytes);
  if (!document.isObject()) {
    return {};
  }

  return document.object();
}

QString normalizedSelectedWorkspaceId(QString workspaceId) {
  const auto normalized = workspaceId.trimmed();
  if (normalized.isEmpty() ||
      normalized.toUtf8().size() > kMaxWorkspaceIdBytes) {
    return {};
  }
  return normalized;
}

QString loadSelectedWorkspaceId(const QString &runtimeDir) {
  return normalizedSelectedWorkspaceId(loadDesktopConfig(runtimeDir)
                                           .value(QStringLiteral("workspaceId"))
                                           .toString());
}

QString loadDefaultPeerEndpoint(const QString &runtimeDir) {
  return loadDesktopConfig(runtimeDir)
      .value(QStringLiteral("defaultPeerEndpoint"))
      .toString();
}

QStringList normalizedPeerEndpoints(QStringList endpoints) {
  QStringList normalized;
  for (const auto &endpoint : endpoints) {
    const auto trimmed = endpoint.trimmed();
    if (trimmed.isEmpty() || normalized.contains(trimmed)) {
      continue;
    }
    normalized.append(trimmed);
  }
  return normalized;
}

constexpr qsizetype kMaxSavedBackupPeerEndpoints = 32;
constexpr qsizetype kMaxMessageMarkdownBytes = 64 * 1024;
constexpr qint64 kMaxAttachmentFileBytes = 128LL * 1024 * 1024;
constexpr qint64 kMaxDeviceKeyPackageFileBytes = 64LL * 1024;
constexpr qsizetype kMaxFfiPathBytes = kMaxDesktopPathBytes;
constexpr qsizetype kMaxPassphraseBytes = kMaxDesktopPassphraseBytes;
constexpr qsizetype kMaxKeyTransferJsonBytes = 256 * 1024;
constexpr qsizetype kMaxRecoveryBundleJsonBytes = 4 * 1024 * 1024;
constexpr qsizetype kMaxSearchQueryBytes = 512;
constexpr qsizetype kMaxWorkspaceNameBytes = 128;
constexpr qsizetype kMaxChannelNameBytes = 128;
constexpr qsizetype kMaxChannelIdBytes = 128;
constexpr qsizetype kMaxMessageIdBytes = 128;
constexpr qsizetype kMaxDeviceKeyPackageIdBytes = 128;
constexpr qsizetype kMaxEventIdBytes = 68;
constexpr qsizetype kMaxWorkspaceRoleBytes = 16;
constexpr qsizetype kEventIdHashHexBytes = 64;
constexpr qsizetype kMaxDeviceDisplayNameBytes = 128;
constexpr qsizetype kMaxDeviceIdReferenceBytes = 512;
constexpr qsizetype kMaxDeviceKeyPackageProtocolBytes = 128;
constexpr qsizetype kMaxAttachmentSelectorBytes = 256;
constexpr qsizetype kMaxAttachmentMediaTypeBytes = 128;
constexpr qsizetype kMaxPeerEndpointIdBytes = 2304;
constexpr qsizetype kMaxPeerEndpointBytes = 2048;
constexpr qsizetype kMaxBackupPeerEndpointListTextBytes =
    kMaxSavedBackupPeerEndpoints * (kMaxPeerEndpointBytes + 1);
constexpr qsizetype kMaxDirectPeerEndpointListSize =
    kMaxSavedBackupPeerEndpoints + 1;
constexpr qsizetype kMaxPeerEndpointTransportBytes = 64;
constexpr qsizetype kMaxBackupPeerStatusMessageBytes = 512;
constexpr qsizetype kMaxBackupPeerStatusTimestampBytes = 64;
constexpr int kMaxBackupPeerStatusFailureCount = 32;
constexpr int kMaxBackupPeerStatusCount = 1000000;
constexpr int kMaxBackupPeerStatusSuspectScore = 8;
constexpr qsizetype kMaxReactionTextBytes = 64;

bool validateAttachmentFileForSend(const QString &filePath, QString *error) {
  const QFileInfo fileInfo(filePath);
  if (!fileInfo.exists()) {
    *error = QStringLiteral("attachment file not found");
    return false;
  }
  if (!fileInfo.isFile()) {
    *error = QStringLiteral("attachment must be a file");
    return false;
  }
  if (fileInfo.size() > kMaxAttachmentFileBytes) {
    *error = QStringLiteral("attachment file is too large (max 128 MB)");
    return false;
  }
  return true;
}

bool validateDeviceKeyPackageFileForPublish(const QString &filePath,
                                            QString *error) {
  const QFileInfo fileInfo(filePath);
  if (!fileInfo.exists()) {
    *error = QStringLiteral("key package file not found");
    return false;
  }
  if (!fileInfo.isFile()) {
    *error = QStringLiteral("key package must be a file");
    return false;
  }
  if (fileInfo.size() > kMaxDeviceKeyPackageFileBytes) {
    *error = QStringLiteral("key package file is too large (max 64 KB)");
    return false;
  }
  return true;
}

bool validateJsonTextForImport(const QString &json, qsizetype maxBytes,
                               const QString &label, const QString &maxLabel,
                               QString *error) {
  if (json.toUtf8().size() > maxBytes) {
    *error = QStringLiteral("%1 is too large (max %2)").arg(label, maxLabel);
    return false;
  }
  return true;
}

bool validateMetadataTextForWrite(const QString &value, qsizetype maxBytes,
                                  const QString &label, const QString &maxLabel,
                                  QString *error) {
  if (value.toUtf8().size() > maxBytes) {
    *error = QStringLiteral("%1 is too large (max %2)").arg(label, maxLabel);
    return false;
  }
  return true;
}

bool isCanonicalEventId(const QString &value) {
  if (!value.startsWith(QStringLiteral("evt_"))) {
    return false;
  }
  const auto hash = value.mid(4);
  if (hash.size() != kEventIdHashHexBytes) {
    return false;
  }
  for (const auto ch : hash) {
    const auto codepoint = ch.unicode();
    if (!((codepoint >= u'0' && codepoint <= u'9') ||
          (codepoint >= u'a' && codepoint <= u'f'))) {
      return false;
    }
  }
  return true;
}

bool validateCanonicalEventIdForWrite(const QString &value,
                                      const QString &label, QString *error) {
  if (!validateMetadataTextForWrite(value, kMaxEventIdBytes, label,
                                    QStringLiteral("68 bytes"), error)) {
    return false;
  }
  if (!isCanonicalEventId(value)) {
    *error = QStringLiteral("%1 must be canonical").arg(label);
    return false;
  }
  return true;
}

bool validateOpenMlsValueForWrite(const QString &value, bool allowEmptyValue,
                                  QString *error) {
  if (value.isEmpty()) {
    return allowEmptyValue;
  }

  if (allowEmptyValue) {
    return validateCanonicalEventIdForWrite(
        value, QStringLiteral("source event ID"), error);
  }

  return validateMetadataTextForWrite(value, kMaxDeviceKeyPackageIdBytes,
                                      QStringLiteral("key package ID"),
                                      QStringLiteral("128 bytes"), error);
}

enum class PeerEndpointRoute { Unsupported, DirectTcp, NativeIrohDirect };

bool containsAsciiWhitespace(const QString &value) {
  for (const auto ch : value) {
    if (ch.unicode() <= 0x7f && ch.isSpace()) {
      return true;
    }
  }
  return false;
}

bool isValidTcpPort(const QString &port, bool allowZeroPort) {
  if (port.isEmpty()) {
    return false;
  }
  for (const auto ch : port) {
    if (!ch.isDigit()) {
      return false;
    }
  }
  bool ok = false;
  const auto parsed = port.toUInt(&ok);
  return ok && parsed <= 65535 && (allowZeroPort || parsed > 0);
}

bool isValidDirectTcpHost(const QString &host) {
  if (host.isEmpty()) {
    return false;
  }
  for (const auto ch : host) {
    const auto codepoint = ch.unicode();
    const auto asciiAlpha =
        (codepoint >= u'a' && codepoint <= u'z') ||
        (codepoint >= u'A' && codepoint <= u'Z');
    const auto asciiDigit = codepoint >= u'0' && codepoint <= u'9';
    if (!asciiAlpha && !asciiDigit && codepoint != u'.' && codepoint != u'-' &&
        codepoint != u'_') {
      return false;
    }
  }
  return true;
}

bool splitHostPort(const QString &address, QString *host, QString *port,
                   bool *bracketedIpv6) {
  const auto normalized = address.trimmed();
  if (normalized.isEmpty() || containsAsciiWhitespace(normalized)) {
    return false;
  }

  if (normalized.startsWith(QLatin1Char('['))) {
    const auto separator = normalized.indexOf(QStringLiteral("]:"));
    if (separator <= 1) {
      return false;
    }
    *host = normalized.mid(1, separator - 1);
    *port = normalized.mid(separator + 2);
    *bracketedIpv6 = true;
    return !host->isEmpty() && !port->isEmpty();
  }

  const auto separator = normalized.lastIndexOf(QLatin1Char(':'));
  if (separator <= 0 || separator == normalized.size() - 1) {
    return false;
  }
  *host = normalized.left(separator);
  *port = normalized.mid(separator + 1);
  *bracketedIpv6 = false;
  return !host->contains(QLatin1Char(':'));
}

bool directTcpAddressIsValid(const QString &address, bool allowZeroPort) {
  QString host;
  QString port;
  auto bracketedIpv6 = false;
  if (!splitHostPort(address, &host, &port, &bracketedIpv6)) {
    return false;
  }
  if (!isValidTcpPort(port, allowZeroPort)) {
    return false;
  }

  if (bracketedIpv6) {
    QHostAddress parsed;
    return parsed.setAddress(host) &&
           parsed.protocol() == QAbstractSocket::IPv6Protocol;
  }
  return isValidDirectTcpHost(host);
}

bool directTcpPeerEndpointAddressIsValid(const QString &address) {
  return directTcpAddressIsValid(address, false);
}

bool directTcpPeerListenAddressIsValid(const QString &address) {
  return directTcpAddressIsValid(address, true);
}

bool nativeIrohDirectAddrIsValid(const QString &address) {
  QString host;
  QString port;
  auto bracketedIpv6 = false;
  if (!splitHostPort(address, &host, &port, &bracketedIpv6) ||
      !isValidTcpPort(port, false)) {
    return false;
  }

  QHostAddress parsed;
  return parsed.setAddress(host) &&
         parsed.protocol() == (bracketedIpv6 ? QAbstractSocket::IPv6Protocol
                                             : QAbstractSocket::IPv4Protocol);
}

bool nativeIrohEndpointIdSyntaxIsValid(const QString &endpointId) {
  const auto bytes = endpointId.toUtf8();
  if (bytes.size() == 64) {
    return std::all_of(bytes.begin(), bytes.end(), [](const char byte) {
      return (byte >= '0' && byte <= '9') || (byte >= 'a' && byte <= 'f');
    });
  }
  if (bytes.size() != 52) {
    return false;
  }
  return std::all_of(bytes.begin(), bytes.end(), [](const char byte) {
    return (byte >= 'a' && byte <= 'z') || (byte >= 'A' && byte <= 'Z') ||
           (byte >= '2' && byte <= '7');
  });
}

PeerEndpointRoute supportedPeerEndpointRoute(const QString &endpoint) {
  const auto normalized = endpoint.trimmed();
  if (normalized.isEmpty()) {
    return PeerEndpointRoute::Unsupported;
  }

  if (normalized.startsWith(QStringLiteral("direct+tcp://"))) {
    return directTcpPeerEndpointAddressIsValid(normalized.mid(13))
               ? PeerEndpointRoute::DirectTcp
               : PeerEndpointRoute::Unsupported;
  }
  if (normalized.startsWith(QStringLiteral("tcp://"))) {
    return directTcpPeerEndpointAddressIsValid(normalized.mid(6))
               ? PeerEndpointRoute::DirectTcp
               : PeerEndpointRoute::Unsupported;
  }

  if (normalized.startsWith(QStringLiteral("iroh://"))) {
    const auto rest = normalized.mid(7);
    const auto querySeparator = rest.indexOf(QLatin1Char('?'));
    if (querySeparator <= 0) {
      return PeerEndpointRoute::Unsupported;
    }
    const auto endpointId = rest.left(querySeparator);
    if (!nativeIrohEndpointIdSyntaxIsValid(endpointId)) {
      return PeerEndpointRoute::Unsupported;
    }
    auto query = rest.mid(querySeparator + 1);
    const auto fragmentSeparator = query.indexOf(QLatin1Char('#'));
    if (fragmentSeparator >= 0) {
      query = query.left(fragmentSeparator);
    }

    auto hasDirectAddr = false;
    for (const auto &parameter :
         query.split(QLatin1Char('&'), Qt::KeepEmptyParts)) {
      const auto equals = parameter.indexOf(QLatin1Char('='));
      const auto key = equals >= 0 ? parameter.left(equals).trimmed()
                                   : parameter.trimmed();
      const auto value =
          equals >= 0 ? parameter.mid(equals + 1).trimmed() : QString();
      if (key == QStringLiteral("relay")) {
        return PeerEndpointRoute::Unsupported;
      }
      if (key != QStringLiteral("addr") || !nativeIrohDirectAddrIsValid(value)) {
        return PeerEndpointRoute::Unsupported;
      }
      hasDirectAddr = true;
    }
    return hasDirectAddr ? PeerEndpointRoute::NativeIrohDirect
                         : PeerEndpointRoute::Unsupported;
  }

  if (normalized.contains(QStringLiteral("://"))) {
    return PeerEndpointRoute::Unsupported;
  }
  return directTcpPeerEndpointAddressIsValid(normalized)
             ? PeerEndpointRoute::DirectTcp
             : PeerEndpointRoute::Unsupported;
}

bool peerEndpointRouteAllowsTransport(PeerEndpointRoute route,
                                      const QString &transport) {
  const auto normalized = transport.trimmed();
  switch (route) {
  case PeerEndpointRoute::DirectTcp:
    return normalized == QStringLiteral("direct-tcp");
  case PeerEndpointRoute::NativeIrohDirect:
    return normalized == QStringLiteral("iroh") ||
           normalized == QStringLiteral("iroh-direct");
  case PeerEndpointRoute::Unsupported:
    return false;
  }
  return false;
}

bool peerEndpointIsSupportedForUse(const QString &endpoint) {
  return supportedPeerEndpointRoute(endpoint) != PeerEndpointRoute::Unsupported;
}

bool validatePeerEndpointForPublish(const QString &endpointId,
                                    const QString &endpoint,
                                    const QString &transport, QString *error) {
  if (!validateMetadataTextForWrite(endpointId, kMaxPeerEndpointIdBytes,
                                    QStringLiteral("endpoint ID"),
                                    QStringLiteral("2304 bytes"), error) ||
      !validateMetadataTextForWrite(endpoint, kMaxPeerEndpointBytes,
                                    QStringLiteral("peer endpoint"),
                                    QStringLiteral("2 KB"), error) ||
      !validateMetadataTextForWrite(transport, kMaxPeerEndpointTransportBytes,
                                    QStringLiteral("endpoint transport"),
                                    QStringLiteral("64 bytes"), error)) {
    return false;
  }

  const auto route = supportedPeerEndpointRoute(endpoint);
  if (route == PeerEndpointRoute::Unsupported) {
    *error = QStringLiteral(
        "peer endpoint must be a direct TCP or native Iroh direct route");
    return false;
  }
  if (!peerEndpointRouteAllowsTransport(route, transport)) {
    *error = QStringLiteral("peer endpoint transport does not match route");
    return false;
  }
  return true;
}

bool validatePeerEndpointForUse(const QString &endpoint, QString *error) {
  if (!validateMetadataTextForWrite(endpoint, kMaxPeerEndpointBytes,
                                    QStringLiteral("peer endpoint"),
                                    QStringLiteral("2 KB"), error)) {
    return false;
  }
  if (!peerEndpointIsSupportedForUse(endpoint)) {
    *error = QStringLiteral(
        "peer endpoint must be a direct TCP or native Iroh direct route");
    return false;
  }
  return true;
}

bool validateDirectListenEndpointForUse(const QString &endpoint,
                                        QString *error) {
  if (!validateMetadataTextForWrite(endpoint, kMaxPeerEndpointBytes,
                                    QStringLiteral("peer endpoint"),
                                    QStringLiteral("2 KB"), error)) {
    return false;
  }
  if (!directTcpPeerListenAddressIsValid(endpoint)) {
    *error =
        QStringLiteral("direct listen endpoint must be host:port with numeric port");
    return false;
  }
  return true;
}

bool validatePeerEndpointListForUse(const QStringList &endpoints,
                                    QString *error) {
  if (endpoints.size() > kMaxDirectPeerEndpointListSize) {
    *error = QStringLiteral("peer endpoint list is too large (max %1)")
                 .arg(static_cast<qlonglong>(kMaxDirectPeerEndpointListSize));
    return false;
  }
  for (const auto &endpoint : endpoints) {
    if (!validatePeerEndpointForUse(endpoint, error)) {
      return false;
    }
  }
  return true;
}

QStringList boundedPeerEndpoints(QStringList endpoints) {
  auto normalized = normalizedPeerEndpoints(std::move(endpoints));
  normalized.erase(std::remove_if(normalized.begin(), normalized.end(),
                                  [](const QString &endpoint) {
                                    return endpoint.toUtf8().size() >
                                               kMaxPeerEndpointBytes ||
                                           !peerEndpointIsSupportedForUse(
                                               endpoint);
                                  }),
                   normalized.end());
  return normalized;
}

QStringList normalizedBackupPeerEndpoints(QStringList endpoints) {
  auto normalized = boundedPeerEndpoints(std::move(endpoints));
  while (normalized.size() > kMaxSavedBackupPeerEndpoints) {
    normalized.removeLast();
  }
  return normalized;
}

QStringList splitPeerEndpointList(QString endpoints) {
  if (endpoints.trimmed().isEmpty() ||
      endpoints.toUtf8().size() > kMaxBackupPeerEndpointListTextBytes) {
    return {};
  }
  endpoints.replace(QLatin1Char(';'), QLatin1Char(','));
  return normalizedPeerEndpoints(
      endpoints.split(QLatin1Char(','), Qt::SkipEmptyParts));
}

QString joinedPeerEndpoints(QStringList endpoints) {
  return boundedPeerEndpoints(std::move(endpoints)).join(QLatin1Char(';'));
}

QString boundedBackupPeerStatusString(const QVariant &value,
                                      qsizetype maxBytes) {
  const auto text = value.toString().trimmed();
  if (text.isEmpty()) {
    return {};
  }
  if (text.toUtf8().size() <= maxBytes) {
    return text;
  }

  auto bounded = text.left(maxBytes);
  while (!bounded.isEmpty() && bounded.toUtf8().size() > maxBytes) {
    bounded.chop(1);
  }
  return bounded.trimmed();
}

QString normalizedBackupPeerStatusTimestamp(const QVariant &value) {
  const auto timestamp = value.toString().trimmed();
  if (timestamp.isEmpty() ||
      timestamp.toUtf8().size() > kMaxBackupPeerStatusTimestampBytes) {
    return {};
  }

  auto parsed = QDateTime::fromString(timestamp, Qt::ISODateWithMs);
  if (!parsed.isValid()) {
    parsed = QDateTime::fromString(timestamp, Qt::ISODate);
  }
  if (!parsed.isValid()) {
    return {};
  }
  return parsed.toUTC().toString(Qt::ISODateWithMs);
}

int boundedBackupPeerStatusInt(const QVariant &value, int maxValue) {
  bool ok = false;
  const auto parsed = value.toInt(&ok);
  if (!ok) {
    return 0;
  }
  return qBound(0, parsed, maxValue);
}

bool variantBoolValue(const QVariant &value, bool defaultValue = false) {
  if (!value.isValid() || value.isNull()) {
    return defaultValue;
  }
  return value.toBool();
}

QString variantStringValue(const QVariant &value,
                           const QString &defaultValue = QString()) {
  if (!value.isValid() || value.isNull()) {
    return defaultValue;
  }
  return value.toString();
}

QVariantMap sanitizeBackupPeerStatus(const QVariantMap &status) {
  QVariantMap sanitized;

  const auto lastMessage =
      boundedBackupPeerStatusString(status.value(QStringLiteral("lastMessage")),
                                    kMaxBackupPeerStatusMessageBytes);
  if (!lastMessage.isEmpty()) {
    sanitized.insert(QStringLiteral("lastMessage"), lastMessage);
  }

  const auto lastAttemptAt = normalizedBackupPeerStatusTimestamp(
      status.value(QStringLiteral("lastAttemptAt")));
  if (!lastAttemptAt.isEmpty()) {
    sanitized.insert(QStringLiteral("lastAttemptAt"), lastAttemptAt);
  }
  const auto lastSuccessAt = normalizedBackupPeerStatusTimestamp(
      status.value(QStringLiteral("lastSuccessAt")));
  if (!lastSuccessAt.isEmpty()) {
    sanitized.insert(QStringLiteral("lastSuccessAt"), lastSuccessAt);
  }
  const auto lastFailureAt = normalizedBackupPeerStatusTimestamp(
      status.value(QStringLiteral("lastFailureAt")));
  if (!lastFailureAt.isEmpty()) {
    sanitized.insert(QStringLiteral("lastFailureAt"), lastFailureAt);
  }
  const auto nextAttemptAfter = normalizedBackupPeerStatusTimestamp(
      status.value(QStringLiteral("nextAttemptAfter")));
  if (!nextAttemptAfter.isEmpty()) {
    sanitized.insert(QStringLiteral("nextAttemptAfter"), nextAttemptAfter);
  }
  const auto lastSuspectAt = normalizedBackupPeerStatusTimestamp(
      status.value(QStringLiteral("lastSuspectAt")));
  if (!lastSuspectAt.isEmpty()) {
    sanitized.insert(QStringLiteral("lastSuspectAt"), lastSuspectAt);
  }
  const auto lastRepairAt = normalizedBackupPeerStatusTimestamp(
      status.value(QStringLiteral("lastRepairAt")));
  if (!lastRepairAt.isEmpty()) {
    sanitized.insert(QStringLiteral("lastRepairAt"), lastRepairAt);
  }

  const auto failureCount =
      boundedBackupPeerStatusInt(status.value(QStringLiteral("failureCount")),
                                 kMaxBackupPeerStatusFailureCount);
  if (failureCount > 0) {
    sanitized.insert(QStringLiteral("failureCount"), failureCount);
  }
  const auto lastMissingBlobCount = boundedBackupPeerStatusInt(
      status.value(QStringLiteral("lastMissingBlobCount")),
      kMaxBackupPeerStatusCount);
  if (lastMissingBlobCount > 0) {
    sanitized.insert(QStringLiteral("lastMissingBlobCount"),
                     lastMissingBlobCount);
  }
  const auto lastSkippedGapCount = boundedBackupPeerStatusInt(
      status.value(QStringLiteral("lastSkippedGapCount")),
      kMaxBackupPeerStatusCount);
  if (lastSkippedGapCount > 0) {
    sanitized.insert(QStringLiteral("lastSkippedGapCount"),
                     lastSkippedGapCount);
  }
  const auto suspectScore =
      boundedBackupPeerStatusInt(status.value(QStringLiteral("suspectScore")),
                                 kMaxBackupPeerStatusSuspectScore);
  if (suspectScore > 0) {
    sanitized.insert(QStringLiteral("suspectScore"), suspectScore);
  }

  if (variantBoolValue(status.value(QStringLiteral("lastPartial")))) {
    sanitized.insert(QStringLiteral("lastPartial"), true);
  }
  if (variantBoolValue(status.value(QStringLiteral("lastSuspectPeer")))) {
    sanitized.insert(QStringLiteral("lastSuspectPeer"), true);
  }

  return sanitized;
}

QString transportLabelForPeerEndpoint(const QString &endpoint) {
  const auto normalized = endpoint.trimmed();
  if (normalized.startsWith(QStringLiteral("iroh://"), Qt::CaseInsensitive)) {
    if (normalized.contains(QStringLiteral("relay="), Qt::CaseInsensitive)) {
      return QStringLiteral("iroh-relay");
    }
    if (!normalized.contains(QStringLiteral("addr="), Qt::CaseInsensitive)) {
      return QStringLiteral("iroh-discovery");
    }
    return QStringLiteral("iroh-direct");
  }
  if (normalized.startsWith(QStringLiteral("iroh+relay://"),
                            Qt::CaseInsensitive) ||
      normalized.startsWith(QStringLiteral("relay://"), Qt::CaseInsensitive)) {
    return QStringLiteral("iroh-relay");
  }
  if (normalized.startsWith(QStringLiteral("iroh+discovery://"),
                            Qt::CaseInsensitive) ||
      normalized.startsWith(QStringLiteral("discovery://"),
                            Qt::CaseInsensitive)) {
    return QStringLiteral("iroh-discovery");
  }
  return QStringLiteral("direct-tcp");
}

QVariantMap pruneBackupPeerStatuses(const QVariantMap &statuses,
                                    const QStringList &backupPeerEndpoints) {
  QVariantMap pruned;
  for (const auto &endpoint :
       normalizedBackupPeerEndpoints(backupPeerEndpoints)) {
    const auto status =
        sanitizeBackupPeerStatus(statuses.value(endpoint).toMap());
    if (!status.isEmpty()) {
      pruned.insert(endpoint, status);
    }
  }
  return pruned;
}

QString currentUtcTimestamp() {
  return QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs);
}

constexpr std::size_t kDefaultTimelineLimit = 500;
constexpr std::size_t kMaxTimelineLimit = 500;
constexpr qsizetype kMaxTimelineLimitTextBytes = 16;

std::size_t configuredTimelineLimit() {
  const auto configuredLimit =
      qEnvironmentVariable("CHAFT_TIMELINE_LIMIT").trimmed();
  if (configuredLimit.isEmpty() ||
      configuredLimit.toUtf8().size() > kMaxTimelineLimitTextBytes) {
    return kDefaultTimelineLimit;
  }
  bool ok = false;
  const auto parsed = configuredLimit.toULongLong(&ok);
  if (!ok) {
    return kDefaultTimelineLimit;
  }
  return std::min<std::size_t>(static_cast<std::size_t>(parsed),
                               kMaxTimelineLimit);
}

std::size_t configuredChannelPageLimit() { return 128; }

std::size_t configuredMemberPageLimit() { return 128; }

bool queryHasSearchTerms(const QString &query) {
  for (const auto &character : query) {
    if (character.isLetterOrNumber()) {
      return true;
    }
  }
  return false;
}

qint64 hostedPeerEndpointExpiresAtMs() {
  constexpr qint64 hostedPeerEndpointTtlMs = 10 * 60 * 1000;
  return QDateTime::currentMSecsSinceEpoch() + hostedPeerEndpointTtlMs;
}

QDateTime parseUtcTimestamp(const QString &timestamp) {
  auto parsed = QDateTime::fromString(timestamp, Qt::ISODateWithMs);
  if (!parsed.isValid()) {
    parsed = QDateTime::fromString(timestamp, Qt::ISODate);
  }
  if (!parsed.isValid()) {
    return {};
  }
  return parsed.toUTC();
}

int backupPeerCooldownSeconds(int failureCount) {
  if (failureCount <= 0) {
    return 0;
  }

  auto seconds = 15;
  for (auto i = 1; i < failureCount; ++i) {
    seconds = qMin(seconds * 2, 300);
  }
  return seconds;
}

bool backupPeerStatusInCooldown(const QVariantMap &status,
                                const QDateTime &now) {
  const auto nextAttemptAt = parseUtcTimestamp(
      status.value(QStringLiteral("nextAttemptAfter")).toString());
  if (nextAttemptAt.isValid()) {
    return now < nextAttemptAt;
  }

  const auto failureCount =
      boundedBackupPeerStatusInt(status.value(QStringLiteral("failureCount")),
                                 kMaxBackupPeerStatusFailureCount);
  const auto lastFailureAt = parseUtcTimestamp(
      status.value(QStringLiteral("lastFailureAt")).toString());
  return failureCount > 0 && lastFailureAt.isValid() &&
         lastFailureAt.secsTo(now) < backupPeerCooldownSeconds(failureCount);
}

int backupPeerSuspectScore(const QVariantMap &status) {
  const auto explicitScore = status.value(QStringLiteral("suspectScore"));
  if (explicitScore.isValid() && !explicitScore.isNull()) {
    return boundedBackupPeerStatusInt(explicitScore,
                                      kMaxBackupPeerStatusSuspectScore);
  }
  return variantBoolValue(status.value(QStringLiteral("lastSuspectPeer"))) ? 1
                                                                            : 0;
}

struct RetryPeerCandidate {
  QString endpoint;
  bool cooling = false;
  bool partial = false;
  int suspectScore = 0;
  int failureCount = 0;
  int missingBlobCount = 0;
  QDateTime nextAttemptAt;
  QDateTime lastSuccessAt;
  QDateTime lastFailureAt;
  qsizetype originalIndex = 0;
};

QStringList orderedBlobRetryPeerEndpoints(
    const QString &explicitPeerEndpoint, const QStringList &backupPeerEndpoints,
    const QVariantMap &backupPeerStatuses, const QDateTime &now) {
  QStringList ordered;
  const auto explicitPeers =
      normalizedPeerEndpoints(QStringList{explicitPeerEndpoint});
  const auto explicitPeer =
      explicitPeers.isEmpty() ? QString() : explicitPeers.first();
  if (!explicitPeer.isEmpty()) {
    ordered.append(explicitPeer);
  }

  QList<RetryPeerCandidate> candidates;
  const auto normalizedBackupPeers =
      normalizedBackupPeerEndpoints(backupPeerEndpoints);
  for (qsizetype index = 0; index < normalizedBackupPeers.size(); ++index) {
    const auto endpoint = normalizedBackupPeers.at(index);
    if (endpoint == explicitPeer) {
      continue;
    }

    const auto status =
        sanitizeBackupPeerStatus(backupPeerStatuses.value(endpoint).toMap());
    candidates.append(RetryPeerCandidate{
        endpoint,
        backupPeerStatusInCooldown(status, now),
        variantBoolValue(status.value(QStringLiteral("lastPartial"))),
        backupPeerSuspectScore(status),
        status.value(QStringLiteral("failureCount")).toInt(0),
        status.value(QStringLiteral("lastMissingBlobCount")).toInt(0),
        parseUtcTimestamp(
            status.value(QStringLiteral("nextAttemptAfter")).toString()),
        parseUtcTimestamp(
            status.value(QStringLiteral("lastSuccessAt")).toString()),
        parseUtcTimestamp(
            status.value(QStringLiteral("lastFailureAt")).toString()),
        index,
    });
  }

  std::stable_sort(
      candidates.begin(), candidates.end(),
      [](const RetryPeerCandidate &left, const RetryPeerCandidate &right) {
        if (left.cooling != right.cooling) {
          return !left.cooling;
        }
        if (left.cooling && right.cooling && left.nextAttemptAt.isValid() &&
            right.nextAttemptAt.isValid() &&
            left.nextAttemptAt != right.nextAttemptAt) {
          return left.nextAttemptAt < right.nextAttemptAt;
        }
        if ((left.suspectScore > 0) != (right.suspectScore > 0)) {
          return left.suspectScore == 0;
        }
        if (left.suspectScore != right.suspectScore) {
          return left.suspectScore < right.suspectScore;
        }
        if (left.partial != right.partial) {
          return left.partial;
        }
        if (left.partial && right.partial &&
            left.missingBlobCount != right.missingBlobCount) {
          return left.missingBlobCount > right.missingBlobCount;
        }
        if (left.failureCount != right.failureCount) {
          return left.failureCount < right.failureCount;
        }
        if (left.lastSuccessAt.isValid() != right.lastSuccessAt.isValid()) {
          return left.lastSuccessAt.isValid();
        }
        if (left.lastSuccessAt.isValid() &&
            left.lastSuccessAt != right.lastSuccessAt) {
          return left.lastSuccessAt > right.lastSuccessAt;
        }
        if (left.lastFailureAt.isValid() != right.lastFailureAt.isValid()) {
          return !left.lastFailureAt.isValid();
        }
        if (left.lastFailureAt.isValid() &&
            left.lastFailureAt != right.lastFailureAt) {
          return left.lastFailureAt < right.lastFailureAt;
        }
        return left.originalIndex < right.originalIndex;
      });

  for (const auto &candidate : candidates) {
    ordered.append(candidate.endpoint);
  }
  return normalizedPeerEndpoints(ordered);
}

bool shouldFallbackFromOpenMlsRemovalError(const QString &error) {
  const auto lower = error.toLower();
  return lower.contains(QStringLiteral("openmls")) &&
         (lower.contains(QStringLiteral("missing")) ||
          lower.contains(QStringLiteral("not found")));
}

bool isRuntimeUnlockError(const QString &error) {
  const auto lower = error.toLower();
  return lower.contains(
             QStringLiteral("encrypted identity passphrase is required")) ||
         lower.contains(
             QStringLiteral("local secret file passphrase is required")) ||
         lower.contains(QStringLiteral("authenticated decryption failed"));
}

QStringList loadBackupPeerEndpoints(const QString &runtimeDir) {
  const auto values = loadDesktopConfig(runtimeDir)
                          .value(QStringLiteral("backupPeerEndpoints"))
                          .toArray();
  QStringList endpoints;
  for (const auto &value : values) {
    endpoints.append(value.toString());
  }
  return normalizedBackupPeerEndpoints(endpoints);
}

QVariantMap loadBackupPeerStatuses(const QString &runtimeDir,
                                   const QStringList &backupPeerEndpoints) {
  const auto statusObject = loadDesktopConfig(runtimeDir)
                                .value(QStringLiteral("backupPeerStatuses"))
                                .toObject();
  QVariantMap statuses;
  for (const auto &endpoint :
       normalizedBackupPeerEndpoints(backupPeerEndpoints)) {
    const auto status = sanitizeBackupPeerStatus(
        statusObject.value(endpoint).toObject().toVariantMap());
    if (!status.isEmpty()) {
      statuses.insert(endpoint, status);
    }
  }
  return statuses;
}

bool parseEnabledFlag(const QString &value) {
  const auto normalized = value.trimmed().toLower();
  return normalized == QStringLiteral("1") ||
         normalized == QStringLiteral("true") ||
         normalized == QStringLiteral("yes") ||
         normalized == QStringLiteral("on");
}

bool loadAutoBackupEnabled(const QString &runtimeDir) {
  return loadDesktopConfig(runtimeDir)
      .value(QStringLiteral("autoBackupEnabled"))
      .toBool(false);
}

void saveDesktopConfig(const QString &runtimeDir, const QString &workspaceId,
                       const QString &defaultPeerEndpoint,
                       const QStringList &backupPeerEndpoints,
                       const QVariantMap &backupPeerStatuses,
                       bool autoBackupEnabled) {
  const auto configPath = desktopConfigPath(runtimeDir);
  if (configPath.isEmpty()) {
    return;
  }
  if (!QDir().mkpath(runtimeDir)) {
    return;
  }

  QJsonObject config;
  if (!workspaceId.isEmpty()) {
    config.insert(QStringLiteral("workspaceId"), workspaceId);
  }
  if (!defaultPeerEndpoint.isEmpty()) {
    config.insert(QStringLiteral("defaultPeerEndpoint"), defaultPeerEndpoint);
  }
  const auto normalizedBackupPeers =
      normalizedBackupPeerEndpoints(backupPeerEndpoints);
  if (!normalizedBackupPeers.isEmpty()) {
    QJsonArray backupPeers;
    for (const auto &endpoint : normalizedBackupPeers) {
      backupPeers.append(endpoint);
    }
    config.insert(QStringLiteral("backupPeerEndpoints"), backupPeers);
  }
  const auto prunedBackupPeerStatuses =
      pruneBackupPeerStatuses(backupPeerStatuses, normalizedBackupPeers);
  if (!prunedBackupPeerStatuses.isEmpty()) {
    config.insert(QStringLiteral("backupPeerStatuses"),
                  QJsonObject::fromVariantMap(prunedBackupPeerStatuses));
  }
  if (autoBackupEnabled) {
    config.insert(QStringLiteral("autoBackupEnabled"), autoBackupEnabled);
  }

  const auto bytes = QJsonDocument(config).toJson(QJsonDocument::Indented);
  if (bytes.size() > kMaxDesktopConfigBytes) {
    return;
  }

  QSaveFile file(configPath);
  if (!file.open(QIODevice::WriteOnly)) {
    return;
  }
  if (file.write(bytes) != static_cast<qint64>(bytes.size())) {
    file.cancelWriting();
    return;
  }
  file.commit();
}

QVariantMap initialWorkspaceSnapshot() {
  return snapshotFromJson(fallbackSnapshotJson());
}

} // namespace

class ChaftController : public QObject {
  Q_OBJECT
  Q_PROPERTY(QVariantMap workspaceSnapshot READ workspaceSnapshot NOTIFY
                 workspaceSnapshotChanged)
  Q_PROPERTY(QVariantList workspaceSummaries READ workspaceSummaries NOTIFY
                 workspaceSummariesChanged)
  Q_PROPERTY(QString selectedWorkspaceId READ selectedWorkspaceId NOTIFY
                 selectedWorkspaceChanged)
  Q_PROPERTY(QString syncStatus READ syncStatus NOTIFY syncStatusChanged)
  Q_PROPERTY(QString defaultPeerEndpoint READ defaultPeerEndpoint WRITE
                 setDefaultPeerEndpoint NOTIFY defaultPeerEndpointChanged)
  Q_PROPERTY(QStringList backupPeerEndpoints READ backupPeerEndpoints NOTIFY
                 backupPeerEndpointsChanged)
  Q_PROPERTY(QVariantMap backupPeerStatuses READ backupPeerStatuses NOTIFY
                 backupPeerStatusesChanged)
  Q_PROPERTY(
      QVariantMap publishQueue READ publishQueue NOTIFY publishQueueChanged)
  Q_PROPERTY(QVariantMap workspaceStorageHealth READ workspaceStorageHealth
                 NOTIFY workspaceStorageHealthChanged)
  Q_PROPERTY(bool autoBackupEnabled READ autoBackupEnabled WRITE
                 setAutoBackupEnabled NOTIFY autoBackupEnabledChanged)
  Q_PROPERTY(bool hasRuntimeWorkspace READ hasRuntimeWorkspace NOTIFY
                 runtimeWorkspaceChanged)
  Q_PROPERTY(bool rawEventStoreMode READ rawEventStoreMode CONSTANT)
  Q_PROPERTY(QString deviceId READ deviceId NOTIFY deviceIdChanged)
  Q_PROPERTY(QVariantList messageSearchHits READ messageSearchHits NOTIFY
                 messageSearchChanged)
  Q_PROPERTY(QString messageSearchQuery READ messageSearchQuery NOTIFY
                 messageSearchChanged)
  Q_PROPERTY(int messageSearchHitCount READ messageSearchHitCount NOTIFY
                 messageSearchChanged)
  Q_PROPERTY(bool messageSearchHasMoreHits READ messageSearchHasMoreHits NOTIFY
                 messageSearchChanged)
  Q_PROPERTY(QVariantList channelSearchResults READ channelSearchResults NOTIFY
                 channelSearchChanged)
  Q_PROPERTY(QString channelSearchQuery READ channelSearchQuery NOTIFY
                 channelSearchChanged)
  Q_PROPERTY(QString hostedPeerEndpoint READ hostedPeerEndpoint NOTIFY
                 hostedPeerChanged)
  Q_PROPERTY(bool peerHosting READ peerHosting NOTIFY hostedPeerChanged)
  Q_PROPERTY(bool peerHostingInFlight READ peerHostingInFlight NOTIFY
                 peerHostingInFlightChanged)
  Q_PROPERTY(bool syncInFlight READ syncInFlight NOTIFY syncInFlightChanged)
  Q_PROPERTY(bool runtimeUnlockRequired READ runtimeUnlockRequired NOTIFY
                 runtimeUnlockChanged)
  Q_PROPERTY(
      bool runtimeUnlocked READ runtimeUnlocked NOTIFY runtimeUnlockChanged)
  Q_PROPERTY(bool runtimeLocked READ runtimeLocked NOTIFY runtimeUnlockChanged)
  Q_PROPERTY(bool runtimeUnlockClearable READ runtimeUnlockClearable NOTIFY
                 runtimeUnlockChanged)
  Q_PROPERTY(QString keyTransferJson READ keyTransferJson NOTIFY
                 keyTransferJsonChanged)
  Q_PROPERTY(bool keyTransferInFlight READ keyTransferInFlight NOTIFY
                 keyTransferInFlightChanged)

public:
  explicit ChaftController(QVariantMap fallbackSnapshot,
                           QObject *parent = nullptr)
      : QObject(parent), m_runtimeDir(defaultRuntimeDir()),
        m_identityFile(qEnvironmentVariable("CHAFT_IDENTITY_FILE")),
        m_identityPassphrase(normalizedEnvironmentPassphrase(
            qEnvironmentVariable("CHAFT_IDENTITY_PASSPHRASE"))),
        m_eventStorePath(normalizedEnvironmentPath(
            qEnvironmentVariable("CHAFT_EVENT_STORE"))),
        m_workspaceId(normalizedSelectedWorkspaceId(
            qEnvironmentVariable("CHAFT_WORKSPACE_ID"))),
        m_workspaceSnapshot(std::move(fallbackSnapshot)) {
    m_identityPassphraseFromEnvironment = !m_identityPassphrase.isEmpty();
    m_rawEventStoreMode =
        !m_eventStorePath.isEmpty() &&
        normalizedEnvironmentPath(qEnvironmentVariable("CHAFT_RUNTIME_DIR"))
            .isEmpty();
    if (m_workspaceId.isEmpty() && !m_rawEventStoreMode) {
      m_workspaceId = loadSelectedWorkspaceId(m_runtimeDir);
    }
    m_defaultPeerEndpoint = qEnvironmentVariable("CHAFT_PEER_ENDPOINT");
    if (m_defaultPeerEndpoint.isEmpty()) {
      m_defaultPeerEndpoint = loadDefaultPeerEndpoint(m_runtimeDir);
    }
    m_defaultPeerEndpoint = m_defaultPeerEndpoint.trimmed();
    QString peerEndpointError;
    if (!m_defaultPeerEndpoint.isEmpty() &&
        !validatePeerEndpointForUse(m_defaultPeerEndpoint,
                                    &peerEndpointError)) {
      m_defaultPeerEndpoint.clear();
    }
    m_backupPeerEndpoints = loadBackupPeerEndpoints(m_runtimeDir);
    const auto configuredBackupPeers =
        qEnvironmentVariable("CHAFT_BACKUP_PEERS");
    if (!configuredBackupPeers.isEmpty()) {
      m_backupPeerEndpoints = normalizedBackupPeerEndpoints(
          m_backupPeerEndpoints + splitPeerEndpointList(configuredBackupPeers));
    }
    m_backupPeerStatuses =
        loadBackupPeerStatuses(m_runtimeDir, m_backupPeerEndpoints);
    m_autoBackupEnabled = loadAutoBackupEnabled(m_runtimeDir);
    const auto configuredAutoBackup = qEnvironmentVariable("CHAFT_AUTO_BACKUP");
    if (!configuredAutoBackup.isEmpty()) {
      m_autoBackupEnabled = parseEnabledFlag(configuredAutoBackup);
    }
    loadFfi();
    if (m_ffiReady) {
      if (!m_workspaceId.isEmpty()) {
        applyWorkspaceLoadingSnapshot(m_workspaceId);
      }
      if (m_rawEventStoreMode) {
        setSyncStatus(QStringLiteral("loading event store..."));
        QMetaObject::invokeMethod(
            this, [this]() { queueStoreSnapshotHydration(); },
            Qt::QueuedConnection);
      } else {
        setSyncStatus(QStringLiteral("loading local runtime..."));
        QMetaObject::invokeMethod(
            this, [this]() { queueRuntimeHydration(); }, Qt::QueuedConnection);
      }
    }
    if (m_syncStatus.isEmpty()) {
      setSyncStatus(hasRuntimeWorkspace() ? QStringLiteral("local event log")
                                          : QStringLiteral("demo workspace"));
    }
  }

  ~ChaftController() override {
    if (!m_peerHostingInFlight) {
      stopLocalPeerBlocking();
    }
  }

  QVariantMap workspaceSnapshot() const { return m_workspaceSnapshot; }
  QVariantList workspaceSummaries() const { return m_workspaceSummaries; }
  QString selectedWorkspaceId() const { return m_workspaceId; }
  QString syncStatus() const { return m_syncStatus; }
  QString defaultPeerEndpoint() const { return m_defaultPeerEndpoint; }
  QStringList backupPeerEndpoints() const { return m_backupPeerEndpoints; }
  QVariantMap backupPeerStatuses() const { return m_backupPeerStatuses; }
  QVariantMap publishQueue() const { return m_publishQueue; }
  QVariantMap workspaceStorageHealth() const {
    return m_workspaceStorageHealth;
  }
  bool autoBackupEnabled() const { return m_autoBackupEnabled; }
  QString deviceId() const { return m_deviceId; }
  QVariantList messageSearchHits() const { return m_messageSearchHits; }
  QString messageSearchQuery() const { return m_messageSearchQuery; }
  int messageSearchHitCount() const { return m_messageSearchHitCount; }
  bool messageSearchHasMoreHits() const { return m_messageSearchHasMoreHits; }
  QVariantList channelSearchResults() const { return m_channelSearchResults; }
  QString channelSearchQuery() const { return m_channelSearchQuery; }
  QString hostedPeerEndpoint() const { return m_hostedPeerEndpoint; }
  bool peerHosting() const { return !m_hostedPeerId.isEmpty(); }
  bool peerHostingInFlight() const { return m_peerHostingInFlight; }
  bool syncInFlight() const { return m_syncInFlight; }
  bool runtimeUnlockRequired() const { return m_runtimeUnlockRequired; }
  bool runtimeUnlocked() const { return !m_identityPassphrase.isEmpty(); }
  bool runtimeLocked() const { return m_runtimeAccessSuspendedUntilUnlock; }
  bool runtimeUnlockClearable() const {
    return !m_identityPassphrase.isEmpty() &&
           !m_identityPassphraseFromEnvironment;
  }
  QString keyTransferJson() const { return m_keyTransferJson; }
  bool keyTransferInFlight() const { return m_keyTransferInFlight; }
  bool hasRuntimeWorkspace() const {
    const auto normalizedWorkspaceId = m_workspaceId.trimmed();
    return m_ffiReady && !m_rawEventStoreMode && !m_runtimeDir.isEmpty() &&
           m_runtimeDir.toUtf8().size() <= kMaxFfiPathBytes &&
           (m_identityFile.isEmpty() ||
            m_identityFile.toUtf8().size() <= kMaxFfiPathBytes) &&
           !normalizedWorkspaceId.isEmpty() &&
           normalizedWorkspaceId.toUtf8().size() <= kMaxWorkspaceIdBytes;
  }
  bool rawEventStoreMode() const { return m_rawEventStoreMode; }

  Q_INVOKABLE bool searchQueryHasTerms(const QString &query) const {
    const auto normalizedQuery = query.trimmed();
    if (normalizedQuery.toUtf8().size() > kMaxSearchQueryBytes) {
      return false;
    }
    return queryHasSearchTerms(normalizedQuery);
  }

  Q_INVOKABLE bool copyText(const QString &text,
                            const QString &label = QString()) {
    const auto normalizedText = text.trimmed();
    const auto normalizedLabel = label.trimmed();
    const auto itemLabel =
        normalizedLabel.isEmpty() ? QStringLiteral("text") : normalizedLabel;
    if (normalizedText.isEmpty()) {
      setSyncStatus(QStringLiteral("%1 unavailable").arg(itemLabel));
      return false;
    }

    auto *clipboard = QGuiApplication::clipboard();
    if (clipboard == nullptr) {
      setSyncStatus(QStringLiteral("clipboard unavailable"));
      return false;
    }

    clipboard->setText(normalizedText, QClipboard::Clipboard);
    setSyncStatus(QStringLiteral("%1 copied").arg(itemLabel));
    return true;
  }

  Q_INVOKABLE bool unlockRuntime(const QString &passphrase) {
    if (!validateRuntimePathsForDispatch()) {
      setRuntimeUnlockRequired(true);
      return false;
    }
    if (passphrase.trimmed().isEmpty()) {
      setRuntimeUnlockRequired(true);
      setSyncStatus(QStringLiteral("passphrase required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            passphrase, kMaxPassphraseBytes, QStringLiteral("passphrase"),
            QStringLiteral("16 KB"), &metadataError)) {
      setRuntimeUnlockRequired(true);
      setSyncStatus(metadataError);
      return false;
    }

    if (!storeRuntimeUnlockPassphrase(passphrase)) {
      return false;
    }
    m_identityPassphrase = passphrase;
    m_identityPassphraseFromEnvironment = false;
    m_runtimeAccessSuspendedUntilUnlock = false;
    setRuntimeUnlockRequired(false);
    emit runtimeUnlockChanged();

    setSyncStatus(QStringLiteral("unlocking runtime..."));
    queueRuntimeHydration();
    return true;
  }

  Q_INVOKABLE void requestRuntimeUnlock() {
    if (!hasRuntimeWorkspace()) {
      return;
    }
    if (m_identityPassphraseFromEnvironment) {
      setSyncStatus(
          QStringLiteral("runtime unlock is provided by environment"));
      return;
    }
    if (!m_identityPassphrase.isEmpty()) {
      return;
    }
    setRuntimeUnlockRequired(true);
    setSyncStatus(QStringLiteral("passphrase required"));
  }

  Q_INVOKABLE void clearRuntimeUnlock() {
    if (m_identityPassphraseFromEnvironment) {
      setSyncStatus(
          QStringLiteral("runtime unlock is provided by environment"));
      return;
    }
    if (m_identityPassphrase.isEmpty() && !m_runtimeUnlockRequired) {
      return;
    }
    clearStoredRuntimeUnlockPassphrase();
    m_identityPassphrase.clear();
    m_identityPassphraseFromEnvironment = false;
    m_runtimeAccessSuspendedUntilUnlock = true;
    clearRuntimeSensitiveViewState();
    setRuntimeUnlockRequired(false);
    emit runtimeUnlockChanged();
    setSyncStatus(QStringLiteral("runtime locked"));
  }

  void setDefaultPeerEndpoint(const QString &peerEndpoint) {
    const auto normalized = peerEndpoint.trimmed();
    if (!normalized.isEmpty()) {
      QString metadataError;
      if (!validatePeerEndpointForUse(normalized, &metadataError)) {
        setSyncStatus(metadataError);
        return;
      }
    }
    if (m_defaultPeerEndpoint == normalized) {
      return;
    }

    m_defaultPeerEndpoint = normalized;
    persistDesktopConfig();
    emit defaultPeerEndpointChanged();
  }

  void setAutoBackupEnabled(bool autoBackupEnabled) {
    if (m_autoBackupEnabled == autoBackupEnabled) {
      return;
    }
    m_autoBackupEnabled = autoBackupEnabled;
    persistDesktopConfig();
    emit autoBackupEnabledChanged();
  }

  Q_INVOKABLE bool addBackupPeerEndpoint(const QString &peerEndpoint) {
    const auto normalized = peerEndpoint.trimmed();
    if (normalized.isEmpty()) {
      setSyncStatus(QStringLiteral("backup peer required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointForPublish(
            QStringLiteral("backup:") + normalized, normalized,
            transportLabelForPeerEndpoint(normalized), &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_backupPeerEndpoints.contains(normalized)) {
      setSyncStatus(QStringLiteral("backup peer already saved"));
      return true;
    }
    if (m_backupPeerEndpoints.size() >= kMaxSavedBackupPeerEndpoints) {
      setSyncStatus(
          QStringLiteral("backup peer limit reached (%1)")
              .arg(static_cast<qlonglong>(kMaxSavedBackupPeerEndpoints)));
      return false;
    }

    m_backupPeerEndpoints.append(normalized);
    m_backupPeerEndpoints =
        normalizedBackupPeerEndpoints(m_backupPeerEndpoints);
    m_backupPeerStatuses =
        pruneBackupPeerStatuses(m_backupPeerStatuses, m_backupPeerEndpoints);
    persistDesktopConfig();
    emit backupPeerEndpointsChanged();
    emit backupPeerStatusesChanged();
    if (!m_runtimeAccessSuspendedUntilUnlock && !m_runtimeUnlockRequired &&
        hasRuntimeWorkspace() && m_publishPeerEndpointJson != nullptr) {
      const auto generation = ++m_runtimeWriteGeneration;
      runPeerEndpointPublish(
          QStringLiteral("backup:") + normalized, normalized,
          transportLabelForPeerEndpoint(normalized), true, false, 0,
          QStringLiteral("backup peer saved and announced"), generation,
          QStringLiteral("full_history_with_blobs"),
          QStringLiteral("operator_saved"));
    } else if (m_runtimeAccessSuspendedUntilUnlock || m_runtimeUnlockRequired) {
      setSyncStatus(QStringLiteral("backup peer saved; unlock to announce"));
    } else {
      setSyncStatus(QStringLiteral("backup peer saved"));
    }
    return true;
  }

  Q_INVOKABLE bool removeBackupPeerEndpoint(const QString &peerEndpoint) {
    const auto normalized = peerEndpoint.trimmed();
    const auto removed = m_backupPeerEndpoints.removeAll(normalized);
    if (removed == 0) {
      return false;
    }

    if (!m_backupPeerEndpoints.isEmpty()) {
      m_nextBackupPeerIndex %= m_backupPeerEndpoints.size();
    } else {
      m_nextBackupPeerIndex = 0;
    }
    m_backupPeerStatuses.remove(normalized);
    persistDesktopConfig();
    emit backupPeerEndpointsChanged();
    emit backupPeerStatusesChanged();
    setSyncStatus(QStringLiteral("backup peer removed"));
    return true;
  }

  Q_INVOKABLE bool selectWorkspace(const QString &workspaceId) {
    if (!ensureFfiReady()) {
      return false;
    }

    const auto normalizedWorkspaceId = workspaceId.trimmed();
    if (normalizedWorkspaceId.isEmpty()) {
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedWorkspaceId, kMaxWorkspaceIdBytes,
            QStringLiteral("workspace ID"), QStringLiteral("128 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_workspaceId == normalizedWorkspaceId) {
      setSyncStatus(QStringLiteral("refreshing workspace..."));
      if (m_rawEventStoreMode) {
        queueStoreSnapshotHydration();
      } else {
        queueRuntimeHydration();
      }
      return true;
    }

    const auto hadRuntimeWorkspace = hasRuntimeWorkspace();
    m_workspaceId = normalizedWorkspaceId;
    m_messageSearchQuery.clear();
    m_messageSearchHits.clear();
    m_messageSearchHitCount = 0;
    m_messageSearchHasMoreHits = false;
    m_channelSearchQuery.clear();
    m_channelSearchResults.clear();
    clearWorkspaceStorageHealth();
    emit channelSearchChanged();
    if (!m_rawEventStoreMode) {
      persistDesktopConfig();
    }
    applyWorkspaceLoadingSnapshot(normalizedWorkspaceId);
    emit selectedWorkspaceChanged();
    emit messageSearchChanged();
    if (!hadRuntimeWorkspace) {
      emit runtimeWorkspaceChanged();
    }
    setSyncStatus(QStringLiteral("loading workspace..."));
    if (m_rawEventStoreMode) {
      queueStoreSnapshotHydration();
    } else {
      queueRuntimeHydration();
    }
    return true;
  }

  Q_INVOKABLE bool createWorkspace(const QString &name,
                                   const QString &defaultChannelName) {
    if (!ensureRuntimeAccessReady()) {
      return false;
    }

    const auto workspaceName =
        name.trimmed().isEmpty() ? QStringLiteral("Chaft") : name.trimmed();
    const auto channelName = defaultChannelName.trimmed().isEmpty()
                                 ? QStringLiteral("general")
                                 : defaultChannelName.trimmed();
    QString metadataError;
    if (!validateMetadataTextForWrite(workspaceName, kMaxWorkspaceNameBytes,
                                      QStringLiteral("workspace name"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(
            channelName, kMaxChannelNameBytes, QStringLiteral("channel name"),
            QStringLiteral("128 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_createWorkspaceJson == nullptr) {
      setSyncStatus(QStringLiteral("workspace creation unavailable"));
      return false;
    }

    const auto hadRuntimeWorkspace = hasRuntimeWorkspace();
    const auto previousWorkspaceId = m_workspaceId;
    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("creating workspace..."));
    runWorkspaceCreate(workspaceName, channelName, previousWorkspaceId,
                       hadRuntimeWorkspace, generation);
    return true;
  }

  Q_INVOKABLE bool createChannel(const QString &name, bool isPrivate) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto channelName = name.trimmed();
    if (channelName.isEmpty()) {
      setSyncStatus(QStringLiteral("channel name required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            channelName, kMaxChannelNameBytes, QStringLiteral("channel name"),
            QStringLiteral("128 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("creating channel..."));
    runChannelCreate(channelName, isPrivate, generation);
    return true;
  }

  Q_INVOKABLE bool loadMoreChannels() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_channelPageInFlight) {
      return false;
    }
    if (m_listWorkspaceChannelPageJson == nullptr) {
      setSyncStatus(QStringLiteral("channel paging unavailable"));
      return false;
    }

    const auto channels =
        m_workspaceSnapshot.value(QStringLiteral("channels")).toList();
    const auto channelCount =
        m_workspaceSnapshot.value(QStringLiteral("channelCount")).toULongLong();
    const auto startIndex = static_cast<std::size_t>(channels.size());
    if (channelCount > 0 &&
        static_cast<qulonglong>(startIndex) >= channelCount) {
      setSyncStatus(QStringLiteral("all channels loaded"));
      return false;
    }

    const auto generation = ++m_channelPageGeneration;
    setChannelPageInFlight(true);
    setSyncStatus(QStringLiteral("loading channels..."));
    runWorkspaceChannelPageLoad(startIndex, configuredChannelPageLimit(),
                                m_workspaceId, generation);
    return true;
  }

  Q_INVOKABLE bool loadChannelPageContaining(const QString &channelId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    const auto normalizedChannelId = channelId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("channel required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_channelPageInFlight) {
      return false;
    }
    if (m_listWorkspaceChannelPageContainingJson == nullptr) {
      setSyncStatus(QStringLiteral("channel lookup unavailable"));
      return false;
    }

    const auto generation = ++m_channelPageGeneration;
    setChannelPageInFlight(true);
    setSyncStatus(QStringLiteral("loading channel..."));
    runWorkspaceChannelPageContainingLoad(normalizedChannelId,
                                          configuredChannelPageLimit(),
                                          m_workspaceId, generation);
    return true;
  }

  Q_INVOKABLE bool updateDeviceProfile(const QString &displayName) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedDisplayName = displayName.trimmed();
    if (normalizedDisplayName.isEmpty()) {
      setSyncStatus(QStringLiteral("display name required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedDisplayName, kMaxDeviceDisplayNameBytes,
            QStringLiteral("display name"), QStringLiteral("128 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_updateDeviceProfileJson == nullptr) {
      setSyncStatus(QStringLiteral("profile update unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("updating profile..."));
    runDeviceProfileUpdate(normalizedDisplayName, generation);
    return true;
  }

  Q_INVOKABLE bool sendMessage(const QString &channelId, const QString &text) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    const auto normalizedChannelId = channelId.trimmed();
    const auto trimmedText = text.trimmed();
    if (normalizedChannelId.isEmpty() || trimmedText.isEmpty()) {
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (trimmedText.toUtf8().size() > kMaxMessageMarkdownBytes) {
      setSyncStatus(QStringLiteral("message is too large (max 64 KB)"));
      return false;
    }
    if (m_sendMessageJson == nullptr) {
      setSyncStatus(QStringLiteral("message send unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("sending message..."));
    runMessageSend(normalizedChannelId, QString(), trimmedText, generation);
    return true;
  }

  Q_INVOKABLE bool sendMessageReply(const QString &channelId,
                                    const QString &replyToMessageId,
                                    const QString &text) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    const auto normalizedChannelId = channelId.trimmed();
    const auto trimmedText = text.trimmed();
    const auto normalizedReplyTo = replyToMessageId.trimmed();
    if (normalizedChannelId.isEmpty() || normalizedReplyTo.isEmpty() ||
        trimmedText.isEmpty()) {
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(normalizedReplyTo, kMaxMessageIdBytes,
                                      QStringLiteral("message ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (trimmedText.toUtf8().size() > kMaxMessageMarkdownBytes) {
      setSyncStatus(QStringLiteral("message is too large (max 64 KB)"));
      return false;
    }
    if (m_sendMessageReplyJson == nullptr) {
      setSyncStatus(QStringLiteral("message replies unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("sending reply..."));
    runMessageSend(normalizedChannelId, normalizedReplyTo, trimmedText,
                   generation);
    return true;
  }

  Q_INVOKABLE bool sendAttachment(const QString &channelId, const QString &text,
                                  const QString &filePath,
                                  const QString &mediaType) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("operation already running"));
      return false;
    }

    const auto normalizedFilePath = filePath.trimmed();
    const auto normalizedChannelId = channelId.trimmed();
    const auto normalizedMediaType = mediaType.trimmed();
    const auto trimmedText = text.trimmed();
    if (normalizedChannelId.isEmpty() || normalizedFilePath.isEmpty()) {
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedFilePath, kMaxFfiPathBytes, QStringLiteral("file path"),
            QStringLiteral("64 KB"), &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedMediaType, kMaxAttachmentMediaTypeBytes,
            QStringLiteral("attachment media type"),
            QStringLiteral("128 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (trimmedText.toUtf8().size() > kMaxMessageMarkdownBytes) {
      setSyncStatus(QStringLiteral("message is too large (max 64 KB)"));
      return false;
    }
    QString fileError;
    if (!validateAttachmentFileForSend(normalizedFilePath, &fileError)) {
      setSyncStatus(fileError);
      return false;
    }
    if (m_sendAttachmentJson == nullptr) {
      setSyncStatus(QStringLiteral("attachments unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("sending attachment..."));
    runAttachmentSend(normalizedChannelId, QString(), trimmedText,
                      normalizedFilePath, normalizedMediaType, generation);
    return true;
  }

  Q_INVOKABLE bool sendAttachmentReply(const QString &channelId,
                                       const QString &replyToMessageId,
                                       const QString &text,
                                       const QString &filePath,
                                       const QString &mediaType) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("operation already running"));
      return false;
    }

    const auto normalizedChannelId = channelId.trimmed();
    const auto normalizedReplyTo = replyToMessageId.trimmed();
    const auto normalizedFilePath = filePath.trimmed();
    const auto normalizedMediaType = mediaType.trimmed();
    const auto trimmedText = text.trimmed();
    if (normalizedChannelId.isEmpty() || normalizedReplyTo.isEmpty() ||
        normalizedFilePath.isEmpty()) {
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedReplyTo, kMaxMessageIdBytes, QStringLiteral("message ID"),
            QStringLiteral("128 bytes"), &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedFilePath, kMaxFfiPathBytes, QStringLiteral("file path"),
            QStringLiteral("64 KB"), &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedMediaType, kMaxAttachmentMediaTypeBytes,
            QStringLiteral("attachment media type"),
            QStringLiteral("128 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (trimmedText.toUtf8().size() > kMaxMessageMarkdownBytes) {
      setSyncStatus(QStringLiteral("message is too large (max 64 KB)"));
      return false;
    }
    QString fileError;
    if (!validateAttachmentFileForSend(normalizedFilePath, &fileError)) {
      setSyncStatus(fileError);
      return false;
    }
    if (m_sendAttachmentReplyJson == nullptr) {
      setSyncStatus(QStringLiteral("attachment replies unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("sending attachment reply..."));
    runAttachmentSend(normalizedChannelId, normalizedReplyTo, trimmedText,
                      normalizedFilePath, normalizedMediaType, generation);
    return true;
  }

  Q_INVOKABLE bool publishDeviceKeyPackage(const QString &protocol,
                                           const QString &filePath) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedProtocol = protocol.trimmed();
    const auto normalizedFilePath = filePath.trimmed();
    if (normalizedProtocol.isEmpty()) {
      setSyncStatus(QStringLiteral("key package protocol required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedProtocol, kMaxDeviceKeyPackageProtocolBytes,
            QStringLiteral("key package protocol"), QStringLiteral("128 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (normalizedFilePath.isEmpty()) {
      setSyncStatus(QStringLiteral("key package file required"));
      return false;
    }
    if (!validateMetadataTextForWrite(normalizedFilePath, kMaxFfiPathBytes,
                                      QStringLiteral("key package file path"),
                                      QStringLiteral("64 KB"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    QString fileError;
    if (!validateDeviceKeyPackageFileForPublish(normalizedFilePath,
                                                &fileError)) {
      setSyncStatus(fileError);
      return false;
    }
    if (m_publishDeviceKeyPackageJson == nullptr) {
      setSyncStatus(QStringLiteral("key package publish unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("publishing key package..."));
    runDeviceKeyPackagePublish(normalizedProtocol, normalizedFilePath,
                               generation);
    return true;
  }

  Q_INVOKABLE bool publishPeerEndpoint(const QString &endpointId,
                                       const QString &endpoint,
                                       const QString &transport,
                                       bool isBackupPeer) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedEndpointId = endpointId.trimmed();
    const auto normalizedEndpoint = endpoint.trimmed();
    const auto normalizedTransport =
        transport.trimmed().isEmpty() ? transportLabelForPeerEndpoint(endpoint)
                                      : transport.trimmed();
    if (normalizedEndpointId.isEmpty()) {
      setSyncStatus(QStringLiteral("endpoint ID required"));
      return false;
    }
    if (normalizedEndpoint.isEmpty()) {
      setSyncStatus(QStringLiteral("peer endpoint required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointForPublish(normalizedEndpointId,
                                        normalizedEndpoint, normalizedTransport,
                                        &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_publishPeerEndpointJson == nullptr) {
      setSyncStatus(QStringLiteral("endpoint publish unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("publishing endpoint..."));
    runPeerEndpointPublish(normalizedEndpointId, normalizedEndpoint,
                           normalizedTransport, isBackupPeer, false, 0,
                           QStringLiteral("endpoint published"), generation,
                           isBackupPeer
                               ? QStringLiteral("full_history_with_blobs")
                               : QString(),
                           isBackupPeer ? QStringLiteral("operator_saved")
                                        : QString());
    return true;
  }

  Q_INVOKABLE bool publishOpenMlsDeviceKeyPackage() {
    return callWorkspaceOpenMlsAction(
        m_publishOpenMlsDeviceKeyPackageJson,
        QStringLiteral("OpenMLS key package unavailable"),
        QStringLiteral("OpenMLS key package published"));
  }

  Q_INVOKABLE bool createOpenMlsWorkspaceGroup() {
    return callWorkspaceOpenMlsAction(
        m_createOpenMlsWorkspaceGroupJson,
        QStringLiteral("OpenMLS workspace group unavailable"),
        QStringLiteral("OpenMLS workspace group ready"));
  }

  Q_INVOKABLE bool addOpenMlsWorkspaceGroupMember(const QString &keyPackageId) {
    return callWorkspaceOpenMlsValueAction(
        m_addOpenMlsWorkspaceGroupMemberJson, keyPackageId, false,
        QStringLiteral("key package ID required"),
        QStringLiteral("OpenMLS workspace member add unavailable"),
        QStringLiteral("OpenMLS workspace member added"));
  }

  Q_INVOKABLE bool
  joinOpenMlsWorkspaceGroup(const QString &sourceEventId = QString()) {
    return callWorkspaceOpenMlsValueAction(
        m_joinOpenMlsWorkspaceGroupJson, sourceEventId, true, QString(),
        QStringLiteral("OpenMLS workspace join unavailable"),
        QStringLiteral("OpenMLS workspace group joined"));
  }

  Q_INVOKABLE bool updateOpenMlsWorkspaceGroup() {
    return callWorkspaceOpenMlsAction(
        m_updateOpenMlsWorkspaceGroupJson,
        QStringLiteral("OpenMLS workspace update unavailable"),
        QStringLiteral("OpenMLS workspace group updated"));
  }

  Q_INVOKABLE bool updateWorkspaceOpenMlsGroups() {
    return callWorkspaceOpenMlsAction(
        m_updateWorkspaceOpenMlsGroupsJson,
        QStringLiteral("OpenMLS group rotation unavailable"),
        QStringLiteral("OpenMLS groups updated"));
  }

  Q_INVOKABLE bool
  applyOpenMlsWorkspaceGroupCommits(const QString &sourceEventId = QString()) {
    return callWorkspaceOpenMlsValueAction(
        m_applyOpenMlsWorkspaceGroupCommitsJson, sourceEventId, true, QString(),
        QStringLiteral("OpenMLS workspace catch-up unavailable"),
        QStringLiteral("OpenMLS workspace commits applied"));
  }

  Q_INVOKABLE bool createOpenMlsChannelGroup(const QString &channelId) {
    return callChannelOpenMlsAction(
        m_createOpenMlsChannelGroupJson, channelId,
        QStringLiteral("OpenMLS channel group unavailable"),
        QStringLiteral("OpenMLS channel group ready"));
  }

  Q_INVOKABLE bool addOpenMlsChannelGroupMember(const QString &channelId,
                                                const QString &keyPackageId) {
    return callChannelOpenMlsValueAction(
        m_addOpenMlsChannelGroupMemberJson, channelId, keyPackageId, false,
        QStringLiteral("key package ID required"),
        QStringLiteral("OpenMLS channel member add unavailable"),
        QStringLiteral("OpenMLS channel member added"));
  }

  Q_INVOKABLE bool
  joinOpenMlsChannelGroup(const QString &channelId,
                          const QString &sourceEventId = QString()) {
    return callChannelOpenMlsValueAction(
        m_joinOpenMlsChannelGroupJson, channelId, sourceEventId, true,
        QString(), QStringLiteral("OpenMLS channel join unavailable"),
        QStringLiteral("OpenMLS channel group joined"));
  }

  Q_INVOKABLE bool updateOpenMlsChannelGroup(const QString &channelId) {
    return callChannelOpenMlsAction(
        m_updateOpenMlsChannelGroupJson, channelId,
        QStringLiteral("OpenMLS channel update unavailable"),
        QStringLiteral("OpenMLS channel group updated"));
  }

  Q_INVOKABLE bool
  applyOpenMlsChannelGroupCommits(const QString &channelId,
                                  const QString &sourceEventId = QString()) {
    return callChannelOpenMlsValueAction(
        m_applyOpenMlsChannelGroupCommitsJson, channelId, sourceEventId, true,
        QString(), QStringLiteral("OpenMLS channel catch-up unavailable"),
        QStringLiteral("OpenMLS channel commits applied"));
  }

  Q_INVOKABLE bool saveAttachment(const QString &messageId,
                                  const QString &attachmentSelector,
                                  const QString &outputPath) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("operation already running"));
      return false;
    }

    const auto normalizedMessageId = messageId.trimmed();
    const auto normalizedAttachmentSelector = attachmentSelector.trimmed();
    const auto normalizedOutputPath = outputPath.trimmed();
    if (normalizedMessageId.isEmpty()) {
      setSyncStatus(QStringLiteral("message ID required"));
      return false;
    }
    if (normalizedAttachmentSelector.isEmpty()) {
      setSyncStatus(QStringLiteral("attachment selector required"));
      return false;
    }
    if (normalizedOutputPath.isEmpty()) {
      setSyncStatus(QStringLiteral("output path required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedMessageId, kMaxMessageIdBytes,
                                      QStringLiteral("message ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedAttachmentSelector, kMaxAttachmentSelectorBytes,
            QStringLiteral("attachment selector"), QStringLiteral("256 bytes"),
            &metadataError) ||
        !validateMetadataTextForWrite(normalizedOutputPath, kMaxFfiPathBytes,
                                      QStringLiteral("output path"),
                                      QStringLiteral("64 KB"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_saveAttachmentJson == nullptr) {
      setSyncStatus(QStringLiteral("attachment save unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("saving attachment..."));
    runAttachmentSave(normalizedMessageId, normalizedAttachmentSelector,
                      normalizedOutputPath);
    return true;
  }

  Q_INVOKABLE bool pruneBlobs() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("operation already running"));
      return false;
    }
    if (m_pruneBlobsJson == nullptr) {
      setSyncStatus(QStringLiteral("blob pruning unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("pruning blobs..."));
    runBlobPrune();
    return true;
  }

  Q_INVOKABLE bool editMessage(const QString &messageId, const QString &text) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedMessageId = messageId.trimmed();
    const auto trimmedText = text.trimmed();
    if (normalizedMessageId.isEmpty()) {
      setSyncStatus(QStringLiteral("message ID required"));
      return false;
    }
    if (trimmedText.isEmpty()) {
      return false;
    }
    if (trimmedText.toUtf8().size() > kMaxMessageMarkdownBytes) {
      setSyncStatus(QStringLiteral("message is too large (max 64 KB)"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedMessageId, kMaxMessageIdBytes,
                                      QStringLiteral("message ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_editMessageJson == nullptr) {
      setSyncStatus(QStringLiteral("message editing unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("editing message..."));
    runMessageEdit(normalizedMessageId, trimmedText, generation);
    return true;
  }

  Q_INVOKABLE bool deleteMessage(const QString &messageId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedMessageId = messageId.trimmed();
    if (normalizedMessageId.isEmpty()) {
      setSyncStatus(QStringLiteral("message ID required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedMessageId, kMaxMessageIdBytes,
                                      QStringLiteral("message ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_deleteMessageJson == nullptr) {
      setSyncStatus(QStringLiteral("message deletion unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("deleting message..."));
    runMessageDelete(normalizedMessageId, generation);
    return true;
  }

  Q_INVOKABLE bool addReaction(const QString &messageId,
                               const QString &reaction) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedMessageId = messageId.trimmed();
    const auto normalizedReaction = reaction.trimmed();
    if (normalizedMessageId.isEmpty()) {
      setSyncStatus(QStringLiteral("message ID required"));
      return false;
    }
    if (normalizedReaction.isEmpty()) {
      setSyncStatus(QStringLiteral("reaction required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedMessageId, kMaxMessageIdBytes,
                                      QStringLiteral("message ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(normalizedReaction, kMaxReactionTextBytes,
                                      QStringLiteral("reaction"),
                                      QStringLiteral("64 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_addReactionJson == nullptr) {
      setSyncStatus(QStringLiteral("reactions unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("adding reaction..."));
    runReactionAdd(normalizedMessageId, normalizedReaction, generation);
    return true;
  }

  Q_INVOKABLE bool removeReaction(const QString &messageId,
                                  const QString &reaction) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedMessageId = messageId.trimmed();
    const auto normalizedReaction = reaction.trimmed();
    if (normalizedMessageId.isEmpty()) {
      setSyncStatus(QStringLiteral("message ID required"));
      return false;
    }
    if (normalizedReaction.isEmpty()) {
      setSyncStatus(QStringLiteral("reaction required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedMessageId, kMaxMessageIdBytes,
                                      QStringLiteral("message ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(normalizedReaction, kMaxReactionTextBytes,
                                      QStringLiteral("reaction"),
                                      QStringLiteral("64 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_removeReactionJson == nullptr) {
      setSyncStatus(QStringLiteral("reaction removal unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("removing reaction..."));
    runReactionRemove(normalizedMessageId, normalizedReaction, generation);
    return true;
  }

  Q_INVOKABLE bool markChannelRead(const QString &channelId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedChannelId = channelId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_markChannelReadJson == nullptr) {
      setSyncStatus(QStringLiteral("read markers unavailable"));
      return false;
    }

    const auto readGeneration = ++m_readMarkerGeneration;
    const auto writeGeneration = ++m_runtimeWriteGeneration;
    runChannelReadMark(normalizedChannelId, readGeneration, writeGeneration);
    return true;
  }

  Q_INVOKABLE bool inviteMember(const QString &deviceId, const QString &role) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedDeviceId = deviceId.trimmed();
    const auto normalizedRole =
        role.trimmed().isEmpty() ? QStringLiteral("member") : role.trimmed();
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("device ID required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("device ID"), QStringLiteral("512 bytes"),
            &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedRole, kMaxWorkspaceRoleBytes,
            QStringLiteral("workspace role"), QStringLiteral("16 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_inviteMemberJson == nullptr) {
      setSyncStatus(QStringLiteral("invite unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("inviting device..."));
    runMemberInvite(normalizedDeviceId, normalizedRole, generation);
    return true;
  }

  Q_INVOKABLE bool removeMember(const QString &deviceId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedDeviceId = deviceId.trimmed();
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("device ID required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("device ID"), QStringLiteral("512 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_removeMemberWithOpenMlsJson == nullptr &&
        m_removeMemberWithKeyRotationJson == nullptr) {
      setSyncStatus(QStringLiteral("member removal unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("removing workspace member..."));
    runMemberRemove(normalizedDeviceId, generation);
    return true;
  }

  Q_INVOKABLE bool loadMoreMembers() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_memberPageInFlight) {
      setSyncStatus(QStringLiteral("member page already loading"));
      return false;
    }
    if (m_listWorkspaceMemberPageJson == nullptr) {
      setSyncStatus(QStringLiteral("member paging unavailable"));
      return false;
    }

    const auto members =
        m_workspaceSnapshot.value(QStringLiteral("members")).toList();
    const auto memberCount =
        m_workspaceSnapshot.value(QStringLiteral("memberCount")).toULongLong();
    const auto startIndex = static_cast<std::size_t>(members.size());
    if (memberCount > 0 && static_cast<qulonglong>(startIndex) >= memberCount) {
      setSyncStatus(QStringLiteral("all members loaded"));
      return false;
    }

    const auto generation = ++m_memberPageGeneration;
    setMemberPageInFlight(true);
    setSyncStatus(QStringLiteral("loading members..."));
    runWorkspaceMemberPageLoad(startIndex, configuredMemberPageLimit(),
                               m_workspaceId, generation);
    return true;
  }

  Q_INVOKABLE bool addChannelMember(const QString &channelId,
                                    const QString &deviceId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedChannelId = channelId.trimmed();
    const auto normalizedDeviceId = deviceId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("channel required"));
      return false;
    }
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("device ID required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("device ID"), QStringLiteral("512 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_addChannelMemberJson == nullptr) {
      setSyncStatus(QStringLiteral("channel grants unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("granting channel access..."));
    runChannelMemberAdd(normalizedChannelId, normalizedDeviceId, generation);
    return true;
  }

  Q_INVOKABLE bool removeChannelMember(const QString &channelId,
                                       const QString &deviceId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedChannelId = channelId.trimmed();
    const auto normalizedDeviceId = deviceId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("channel required"));
      return false;
    }
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("device ID required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("device ID"), QStringLiteral("512 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_removeChannelMemberWithOpenMlsJson == nullptr &&
        m_removeChannelMemberWithKeyRotationJson == nullptr) {
      setSyncStatus(QStringLiteral("channel removal unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("removing channel member..."));
    runChannelMemberRemove(normalizedChannelId, normalizedDeviceId, generation);
    return true;
  }

  Q_INVOKABLE bool exportWorkspaceKey() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("key transfer already running"));
      return false;
    }
    if (m_exportWorkspaceKeyJson == nullptr) {
      setSyncStatus(QStringLiteral("key export unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("exporting workspace key..."));
    runWorkspaceJsonExport(m_exportWorkspaceKeyJson,
                           QStringLiteral("workspace key exported"));
    return true;
  }

  Q_INVOKABLE bool exportTrustSnapshot() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("key transfer already running"));
      return false;
    }
    if (m_exportTrustSnapshotJson == nullptr) {
      setSyncStatus(QStringLiteral("trust snapshot export unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("exporting trust snapshot..."));
    runWorkspaceJsonExport(m_exportTrustSnapshotJson,
                           QStringLiteral("trust snapshot exported"));
    return true;
  }

  Q_INVOKABLE bool rotateWorkspaceManualKeys() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("key transfer already running"));
      return false;
    }
    if (m_rotateWorkspaceForSuspectedCompromiseJson == nullptr) {
      setSyncStatus(QStringLiteral("key rotation unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("rotating keys..."));
    runWorkspaceCompromiseRotation(generation);
    return true;
  }

  Q_INVOKABLE bool detectCompromise() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("key transfer already running"));
      return false;
    }
    if (m_detectCompromiseJson == nullptr) {
      setSyncStatus(QStringLiteral("security review unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("reviewing security..."));
    runWorkspaceCompromiseDetection();
    return true;
  }

  Q_INVOKABLE bool importWorkspaceKey(const QString &keyJson) {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_rawEventStoreMode) {
      setSyncStatus(QStringLiteral("runtime unavailable in event-store view"));
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("key transfer already running"));
      return false;
    }
    const auto normalizedKeyJson = keyJson.trimmed();
    if (normalizedKeyJson.isEmpty()) {
      setSyncStatus(QStringLiteral("workspace key JSON required"));
      return false;
    }
    QString jsonError;
    if (!validateJsonTextForImport(normalizedKeyJson, kMaxKeyTransferJsonBytes,
                                   QStringLiteral("workspace key JSON"),
                                   QStringLiteral("256 KB"), &jsonError)) {
      setSyncStatus(jsonError);
      return false;
    }
    if (m_importWorkspaceKeyJson == nullptr) {
      setSyncStatus(QStringLiteral("key import unavailable"));
      return false;
    }

    const auto hadRuntimeWorkspace = hasRuntimeWorkspace();
    const auto previousWorkspaceId = m_workspaceId;
    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("importing workspace key..."));
    runWorkspaceKeyImport(m_importWorkspaceKeyJson, normalizedKeyJson,
                          previousWorkspaceId, hadRuntimeWorkspace, generation,
                          QStringLiteral("workspace key imported"));
    return true;
  }

  Q_INVOKABLE bool exportChannelKey(const QString &channelId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("key transfer already running"));
      return false;
    }
    const auto normalizedChannelId = channelId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("channel required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_exportChannelKeyJson == nullptr) {
      setSyncStatus(QStringLiteral("channel key export unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("exporting channel key..."));
    runChannelKeyExport(normalizedChannelId,
                        QStringLiteral("channel key exported"));
    return true;
  }

  Q_INVOKABLE bool rotateChannelKey(const QString &channelId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("key transfer already running"));
      return false;
    }
    const auto normalizedChannelId = channelId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("channel required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_rotateChannelKeyJson == nullptr) {
      setSyncStatus(QStringLiteral("channel key rotation unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("rotating channel key..."));
    runChannelKeyRotation(normalizedChannelId, generation);
    return true;
  }

  Q_INVOKABLE bool importChannelKey(const QString &keyJson) {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_rawEventStoreMode) {
      setSyncStatus(QStringLiteral("runtime unavailable in event-store view"));
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("key transfer already running"));
      return false;
    }
    const auto normalizedKeyJson = keyJson.trimmed();
    if (normalizedKeyJson.isEmpty()) {
      setSyncStatus(QStringLiteral("channel key JSON required"));
      return false;
    }
    QString jsonError;
    if (!validateJsonTextForImport(normalizedKeyJson, kMaxKeyTransferJsonBytes,
                                   QStringLiteral("channel key JSON"),
                                   QStringLiteral("256 KB"), &jsonError)) {
      setSyncStatus(jsonError);
      return false;
    }
    if (m_importChannelKeyJson == nullptr) {
      setSyncStatus(QStringLiteral("channel key import unavailable"));
      return false;
    }

    const auto hadRuntimeWorkspace = hasRuntimeWorkspace();
    const auto previousWorkspaceId = m_workspaceId;
    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("importing channel key..."));
    runChannelKeyImport(normalizedKeyJson, previousWorkspaceId,
                        hadRuntimeWorkspace, generation);
    return true;
  }

  Q_INVOKABLE bool exportRecoveryBundle(const QString &passphrase) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("key transfer already running"));
      return false;
    }
    if (passphrase.trimmed().isEmpty()) {
      setSyncStatus(QStringLiteral("recovery passphrase required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(passphrase, kMaxPassphraseBytes,
                                      QStringLiteral("recovery passphrase"),
                                      QStringLiteral("16 KB"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_exportRecoveryBundleJson == nullptr) {
      setSyncStatus(QStringLiteral("recovery bundle export unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("exporting recovery bundle..."));
    runRecoveryBundleExport(passphrase,
                            QStringLiteral("recovery bundle exported"));
    return true;
  }

  Q_INVOKABLE bool importRecoveryBundle(const QString &bundleJson,
                                        const QString &passphrase) {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_rawEventStoreMode) {
      setSyncStatus(QStringLiteral("runtime unavailable in event-store view"));
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("key transfer already running"));
      return false;
    }
    const auto normalizedBundleJson = bundleJson.trimmed();
    if (normalizedBundleJson.isEmpty()) {
      setSyncStatus(QStringLiteral("recovery bundle JSON required"));
      return false;
    }
    QString jsonError;
    if (!validateJsonTextForImport(normalizedBundleJson,
                                   kMaxRecoveryBundleJsonBytes,
                                   QStringLiteral("recovery bundle JSON"),
                                   QStringLiteral("4 MB"), &jsonError)) {
      setSyncStatus(jsonError);
      return false;
    }
    if (passphrase.trimmed().isEmpty()) {
      setSyncStatus(QStringLiteral("recovery passphrase required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(passphrase, kMaxPassphraseBytes,
                                      QStringLiteral("recovery passphrase"),
                                      QStringLiteral("16 KB"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_importRecoveryBundleJson == nullptr) {
      setSyncStatus(QStringLiteral("recovery bundle import unavailable"));
      return false;
    }

    const auto hadRuntimeWorkspace = hasRuntimeWorkspace();
    const auto previousWorkspaceId = m_workspaceId;
    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("importing recovery bundle..."));
    runRecoveryBundleImport(normalizedBundleJson, passphrase,
                            previousWorkspaceId, hadRuntimeWorkspace,
                            generation);
    return true;
  }

  Q_INVOKABLE bool reindexWorkspaceSearch() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("key transfer already running"));
      return false;
    }
    if (m_reindexWorkspaceSearchJson == nullptr) {
      setSyncStatus(QStringLiteral("search reindex unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("reindexing search..."));
    runWorkspaceSearchReindex();
    return true;
  }

  Q_INVOKABLE bool searchWorkspaceMessages(const QString &query) {
    const auto generation = ++m_messageSearchGeneration;
    const auto normalizedQuery = query.trimmed();
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedQuery, kMaxSearchQueryBytes,
                                      QStringLiteral("search query"),
                                      QStringLiteral("512 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!queryHasSearchTerms(normalizedQuery)) {
      if (!m_messageSearchQuery.isEmpty() || !m_messageSearchHits.isEmpty() ||
          m_messageSearchHitCount != 0 || m_messageSearchHasMoreHits) {
        m_messageSearchQuery.clear();
        m_messageSearchHits.clear();
        m_messageSearchHitCount = 0;
        m_messageSearchHasMoreHits = false;
        emit messageSearchChanged();
      }
      return true;
    }

    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_searchWorkspaceJson == nullptr) {
      setSyncStatus(QStringLiteral("search unavailable"));
      return false;
    }

    runWorkspaceSearch(normalizedQuery, generation);
    return true;
  }

  Q_INVOKABLE bool searchWorkspaceChannels(const QString &query) {
    const auto generation = ++m_channelSearchGeneration;
    const auto normalizedQuery = query.trimmed();
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedQuery, kMaxSearchQueryBytes,
                                      QStringLiteral("search query"),
                                      QStringLiteral("512 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!queryHasSearchTerms(normalizedQuery)) {
      if (!m_channelSearchQuery.isEmpty() ||
          !m_channelSearchResults.isEmpty()) {
        m_channelSearchQuery.clear();
        m_channelSearchResults.clear();
        emit channelSearchChanged();
      }
      return true;
    }

    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_searchWorkspaceChannelsJson == nullptr) {
      setSyncStatus(QStringLiteral("channel search unavailable"));
      return false;
    }

    runWorkspaceChannelSearch(normalizedQuery, generation);
    return true;
  }

  Q_INVOKABLE bool startLocalPeer(const QString &listenEndpoint) {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_peerHostingInFlight) {
      setSyncStatus(QStringLiteral("peer hosting already updating"));
      return false;
    }
    if (peerHosting()) {
      setSyncStatus(QStringLiteral("serving %1").arg(m_hostedPeerEndpoint));
      return true;
    }
    if (m_startDirectPeerJson == nullptr) {
      setSyncStatus(QStringLiteral("peer hosting unavailable"));
      return false;
    }

    const auto listen = listenEndpoint.trimmed().isEmpty()
                            ? QStringLiteral("127.0.0.1:0")
                            : listenEndpoint.trimmed();
    QString metadataError;
    if (!validateDirectListenEndpointForUse(listen, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    setSyncStatus(QStringLiteral("starting peer..."));
    runDirectPeerStart(listen);
    return true;
  }

  Q_INVOKABLE bool startLocalIrohPeer() {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_peerHostingInFlight) {
      setSyncStatus(QStringLiteral("peer hosting already updating"));
      return false;
    }
    if (peerHosting()) {
      setSyncStatus(QStringLiteral("serving %1").arg(m_hostedPeerEndpoint));
      return true;
    }
    if (m_startIrohPeerJson == nullptr) {
      setSyncStatus(QStringLiteral("Iroh peer hosting unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("starting Iroh peer..."));
    runIrohPeerStart();
    return true;
  }

  Q_INVOKABLE bool stopLocalPeer() {
    if (m_hostedPeerId.isEmpty()) {
      return true;
    }
    if (m_peerHostingInFlight) {
      setSyncStatus(QStringLiteral("peer hosting already updating"));
      return false;
    }
    if (m_stopDirectPeerJson == nullptr) {
      setSyncStatus(QStringLiteral("peer hosting unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("stopping peer..."));
    runPeerStop(m_hostedPeerId, m_hostedPeerEndpoint, m_hostedPeerEndpointId,
                m_hostedPeerTransport);
    return true;
  }

  Q_INVOKABLE bool refreshHostedPeerEndpointHint() {
    if (!peerHosting() || m_hostedPeerEndpoint.isEmpty() ||
        m_hostedPeerEndpointId.isEmpty() || m_hostedPeerTransport.isEmpty()) {
      return false;
    }

    publishHostedPeerEndpoint(m_hostedPeerEndpointId, m_hostedPeerEndpoint,
                              m_hostedPeerTransport,
                              QStringLiteral("hosted endpoint refreshed"));
    return true;
  }

  Q_INVOKABLE bool publishWorkspace(const QString &peerEndpoint) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("sync already running"));
      return false;
    }
    const auto endpoint = peerEndpoint.trimmed();
    if (endpoint.isEmpty()) {
      setSyncStatus(QStringLiteral("peer endpoint required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointForUse(endpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    setDefaultPeerEndpoint(endpoint);
    setSyncStatus(QStringLiteral("publishing..."));
    runDirectSync(m_publishWorkspaceJson, endpoint, DirectSyncMode::Publish);
    return true;
  }

  Q_INVOKABLE bool backupWorkspace(const QString &peerEndpoint) {
    return startBackupWorkspace(peerEndpoint, true);
  }

  Q_INVOKABLE bool backupWorkspaceIfIdle(const QString &peerEndpoint) {
    if (m_syncInFlight) {
      return false;
    }
    return startBackupWorkspace(peerEndpoint, false);
  }

  Q_INVOKABLE bool backupConfiguredPeersIfIdle() {
    if (m_syncInFlight) {
      return false;
    }
    if (m_backupPeerEndpoints.isEmpty()) {
      return false;
    }

    if (m_nextBackupPeerIndex >= m_backupPeerEndpoints.size()) {
      m_nextBackupPeerIndex = 0;
    }

    const auto now = QDateTime::currentDateTimeUtc();
    auto sawCoolingPeer = false;
    for (qsizetype offset = 0; offset < m_backupPeerEndpoints.size();
         ++offset) {
      const auto index =
          (m_nextBackupPeerIndex + offset) % m_backupPeerEndpoints.size();
      const auto endpoint = m_backupPeerEndpoints.at(index);
      if (backupPeerInCooldown(endpoint, now)) {
        sawCoolingPeer = true;
        continue;
      }

      m_nextBackupPeerIndex = (index + 1) % m_backupPeerEndpoints.size();
      return startBackupWorkspace(endpoint, false);
    }

    if (sawCoolingPeer) {
      setSyncStatus(QStringLiteral("backup peers cooling down"));
    }
    return false;
  }

  Q_INVOKABLE bool publishEventWithTrustSnapshot(const QString &eventId,
                                                 const QString &peerEndpoint) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("sync already running"));
      return false;
    }
    if (m_publishEventWithTrustSnapshotJson == nullptr) {
      setSyncStatus(QStringLiteral("proof publish unavailable"));
      return false;
    }

    const auto normalizedEventId = eventId.trimmed();
    if (normalizedEventId.isEmpty()) {
      setSyncStatus(QStringLiteral("event ID required"));
      return false;
    }
    const auto endpoint = peerEndpoint.trimmed();
    if (endpoint.isEmpty()) {
      setSyncStatus(QStringLiteral("peer endpoint required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedEventId, kMaxEventIdBytes, QStringLiteral("event ID"),
            QStringLiteral("68 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!isCanonicalEventId(normalizedEventId)) {
      setSyncStatus(QStringLiteral("event ID must be canonical"));
      return false;
    }
    if (!validatePeerEndpointForUse(endpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    setDefaultPeerEndpoint(endpoint);
    setSyncStatus(QStringLiteral("publishing proof..."));
    runDirectEventPublishWithTrustSnapshot(normalizedEventId, endpoint);
    return true;
  }

  Q_INVOKABLE bool pullWorkspace(const QString &peerEndpoint) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("sync already running"));
      return false;
    }
    const auto endpoint = peerEndpoint.trimmed();
    if (endpoint.isEmpty()) {
      setSyncStatus(QStringLiteral("peer endpoint required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointForUse(endpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    setDefaultPeerEndpoint(endpoint);
    setSyncStatus(QStringLiteral("pulling..."));
    runDirectSync(m_pullWorkspaceJson, endpoint, DirectSyncMode::Pull);
    return true;
  }

  Q_INVOKABLE bool syncWorkspace(const QString &peerEndpoint) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("sync already running"));
      return false;
    }
    const auto endpoint = peerEndpoint.trimmed();
    if (endpoint.isEmpty()) {
      setSyncStatus(QStringLiteral("peer endpoint required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointForUse(endpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    setDefaultPeerEndpoint(endpoint);
    setSyncStatus(QStringLiteral("syncing..."));
    runDirectSync(m_syncWorkspaceJson, endpoint, DirectSyncMode::Sync);
    return true;
  }

  Q_INVOKABLE bool syncWorkspaceIfIdle(const QString &peerEndpoint) {
    if (m_syncInFlight) {
      return false;
    }
    return syncWorkspace(peerEndpoint);
  }

  Q_INVOKABLE bool repairWorkspaceStorageMetadata() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("sync already running"));
      return false;
    }
    if (m_repairWorkspaceStorageMetadataJson == nullptr) {
      setSyncStatus(QStringLiteral("cache repair unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("repairing cache metadata..."));
    runWorkspaceStorageMetadataRepair();
    return true;
  }

  Q_INVOKABLE bool loadOlderTimeline() {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("sync already running"));
      return false;
    }
    if (m_rawEventStoreMode) {
      if (!validateRawEventStorePathForDispatch()) {
        return false;
      }
      QString workspaceId;
      if (!selectedWorkspaceIdForDispatch(
              &workspaceId, false, QStringLiteral("workspace ID required"))) {
        return false;
      }
      if (m_storeSnapshotWindowJson == nullptr) {
        setSyncStatus(
            QStringLiteral("event store timeline paging unavailable"));
        return false;
      }
    } else {
      if (!ensureRuntimeWorkspace()) {
        return false;
      }
      if (m_runtimeSnapshotWindowJson == nullptr) {
        setSyncStatus(QStringLiteral("timeline paging unavailable"));
        return false;
      }
    }
    const auto currentWindow =
        m_workspaceSnapshot.value(QStringLiteral("timelineWindow")).toMap();
    const auto currentStart =
        currentWindow.value(QStringLiteral("startIndex")).toULongLong();
    if (currentStart == 0) {
      setSyncStatus(QStringLiteral("all history loaded"));
      return false;
    }

    const auto pageLimit = static_cast<qulonglong>(configuredTimelineLimit());
    const auto nextStart =
        currentStart > pageLimit ? currentStart - pageLimit : 0ULL;
    const auto nextCount = currentStart - nextStart;
    if (nextCount == 0) {
      setSyncStatus(QStringLiteral("all history loaded"));
      return false;
    }

    setSyncStatus(QStringLiteral("loading older history..."));
    const auto generation = ++m_timelinePageGeneration;
    if (m_rawEventStoreMode) {
      runStoreTimelinePageLoad(nextStart, nextCount, generation);
    } else {
      runTimelinePageLoad(nextStart, nextCount, generation);
    }
    return true;
  }

  Q_INVOKABLE bool loadChannelTimelineLatest(const QString &channelId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      return false;
    }
    const auto normalizedChannelId = channelId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("channel required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_runtimeChannelSnapshotLatestJson == nullptr) {
      setSyncStatus(QStringLiteral("channel timeline unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("loading channel history..."));
    const auto generation = ++m_timelinePageGeneration;
    runChannelTimelineLatestLoad(normalizedChannelId, configuredTimelineLimit(),
                                 generation);
    return true;
  }

  Q_INVOKABLE bool loadOlderChannelTimeline(const QString &channelId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("sync already running"));
      return false;
    }
    const auto normalizedChannelId = channelId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("channel required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_runtimeChannelSnapshotWindowJson == nullptr) {
      setSyncStatus(QStringLiteral("channel timeline paging unavailable"));
      return false;
    }

    const auto currentWindow =
        m_workspaceSnapshot.value(QStringLiteral("timelineWindow")).toMap();
    const auto currentStart =
        currentWindow.value(QStringLiteral("startIndex")).toULongLong();
    if (currentStart == 0) {
      setSyncStatus(QStringLiteral("all history loaded"));
      return false;
    }

    const auto pageLimit = static_cast<qulonglong>(configuredTimelineLimit());
    const auto nextStart =
        currentStart > pageLimit ? currentStart - pageLimit : 0ULL;
    const auto nextCount = currentStart - nextStart;
    if (nextCount == 0) {
      setSyncStatus(QStringLiteral("all history loaded"));
      return false;
    }

    setSyncStatus(QStringLiteral("loading older channel history..."));
    const auto generation = ++m_timelinePageGeneration;
    runChannelTimelinePageLoad(normalizedChannelId, nextStart, nextCount,
                               generation);
    return true;
  }

  Q_INVOKABLE bool retryBlobTransfers(const QString &peerEndpoint) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("sync already running"));
      return false;
    }
    if (m_retryBlobTransfersJson == nullptr) {
      setSyncStatus(QStringLiteral("blob retry unavailable"));
      return false;
    }

    const auto endpoint = peerEndpoint.trimmed();
    if (!endpoint.isEmpty()) {
      QString metadataError;
      if (!validatePeerEndpointForUse(endpoint, &metadataError)) {
        setSyncStatus(metadataError);
        return false;
      }
      setDefaultPeerEndpoint(endpoint);
    }
    const auto peerEndpoints = orderedBlobRetryPeerEndpoints(
        endpoint, m_backupPeerEndpoints, m_backupPeerStatuses,
        QDateTime::currentDateTimeUtc());
    if (peerEndpoints.isEmpty()) {
      setSyncStatus(QStringLiteral("peer endpoint required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointListForUse(peerEndpoints, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    setSyncStatus(QStringLiteral("retrying blobs..."));
    runBlobTransferRetry(peerEndpoints);
    return true;
  }

signals:
  void workspaceSnapshotChanged();
  void workspaceSummariesChanged();
  void selectedWorkspaceChanged();
  void syncStatusChanged();
  void defaultPeerEndpointChanged();
  void backupPeerEndpointsChanged();
  void backupPeerStatusesChanged();
  void publishQueueChanged();
  void workspaceStorageHealthChanged();
  void autoBackupEnabledChanged();
  void runtimeWorkspaceChanged();
  void deviceIdChanged();
  void messageSearchChanged();
  void channelSearchChanged();
  void hostedPeerChanged();
  void peerHostingInFlightChanged();
  void syncInFlightChanged();
  void runtimeUnlockChanged();
  void keyTransferJsonChanged();
  void keyTransferInFlightChanged();

private:
  static const char *nullableUtf8(const QByteArray &value) {
    return value.isEmpty() ? nullptr : value.constData();
  }

  bool storeRuntimeUnlockPassphrase(const QString &passphrase) {
    if (m_setIdentityPassphrase == nullptr) {
      setRuntimeUnlockRequired(true);
      setSyncStatus(QStringLiteral("runtime unlock cache unavailable"));
      return false;
    }
    if (!validateRuntimeDataDirForDispatch()) {
      setRuntimeUnlockRequired(true);
      return false;
    }

    const auto runtimeDirBytes = m_runtimeDir.toUtf8();
    const auto passphraseBytes = passphrase.toUtf8();
    if (!m_setIdentityPassphrase(runtimeDirBytes.constData(),
                                 passphraseBytes.constData())) {
      setRuntimeUnlockRequired(true);
      setSyncStatus(QStringLiteral("runtime unlock failed"));
      return false;
    }
    return true;
  }

  void clearStoredRuntimeUnlockPassphrase() {
    if (m_clearIdentityPassphrase != nullptr) {
      if (!validateRuntimeDataDirForDispatch()) {
        return;
      }
      const auto runtimeDirBytes = m_runtimeDir.toUtf8();
      m_clearIdentityPassphrase(runtimeDirBytes.constData());
    }
  }

  void handleRuntimeUnlockFailure(const QString &error) {
    if (!isRuntimeUnlockError(error)) {
      return;
    }

    clearStoredRuntimeUnlockPassphrase();
    if (!m_identityPassphrase.isEmpty()) {
      m_identityPassphrase.clear();
      m_identityPassphraseFromEnvironment = false;
      emit runtimeUnlockChanged();
    }
    setRuntimeUnlockRequired(true);
  }

  void clearRuntimeSensitiveViewState() {
    m_lastAppliedRuntimeWriteGeneration = ++m_runtimeWriteGeneration;
    ++m_messageSearchGeneration;
    ++m_channelSearchGeneration;
    auto *clipboard = QGuiApplication::clipboard();
    if (clipboard != nullptr && clipboard->ownsClipboard()) {
      clipboard->clear(QClipboard::Clipboard);
    }
    if (clipboard != nullptr && clipboard->supportsSelection() &&
        clipboard->ownsSelection()) {
      clipboard->clear(QClipboard::Selection);
    }
    const auto messageSearchStateChanged =
        !m_messageSearchQuery.isEmpty() || !m_messageSearchHits.isEmpty() ||
        m_messageSearchHitCount != 0 || m_messageSearchHasMoreHits;
    const auto channelSearchStateChanged =
        !m_channelSearchQuery.isEmpty() || !m_channelSearchResults.isEmpty();
    m_messageSearchQuery.clear();
    m_messageSearchHits.clear();
    m_messageSearchHitCount = 0;
    m_messageSearchHasMoreHits = false;
    m_channelSearchQuery.clear();
    m_channelSearchResults.clear();
    if (messageSearchStateChanged) {
      emit messageSearchChanged();
    }
    if (channelSearchStateChanged) {
      emit channelSearchChanged();
    }
    setKeyTransferJson(QString());
    if (!m_workspaceId.isEmpty()) {
      applyWorkspaceLoadingSnapshot(m_workspaceId);
    }
  }

  void loadFfi() {
    for (const auto &candidate : ffiLibraryCandidates()) {
      m_library.setFileName(candidate);
      if (!m_library.load()) {
        continue;
      }

      m_freeString = reinterpret_cast<FreeStringFn>(
          m_library.resolve("chaft_string_free"));
      m_setIdentityPassphrase =
          reinterpret_cast<RuntimeSetIdentityPassphraseFn>(
              m_library.resolve("chaft_runtime_set_identity_passphrase"));
      m_clearIdentityPassphrase =
          reinterpret_cast<RuntimeClearIdentityPassphraseFn>(
              m_library.resolve("chaft_runtime_clear_identity_passphrase"));
      m_runtimeSnapshotJson =
          reinterpret_cast<RuntimeSnapshotResultJsonFn>(m_library.resolve(
              "chaft_decrypted_workspace_snapshot_from_runtime_result_json"));
      m_runtimeSnapshotLatestJson =
          reinterpret_cast<RuntimeSnapshotLatestResultJsonFn>(
              m_library.resolve("chaft_decrypted_workspace_snapshot_from_"
                                "runtime_latest_result_json"));
      m_runtimeSnapshotWindowJson =
          reinterpret_cast<RuntimeSnapshotWindowResultJsonFn>(
              m_library.resolve("chaft_decrypted_workspace_snapshot_from_"
                                "runtime_window_result_json"));
      m_runtimeChannelSnapshotLatestJson =
          reinterpret_cast<RuntimeChannelSnapshotLatestResultJsonFn>(
              m_library.resolve("chaft_decrypted_workspace_channel_snapshot_"
                                "from_runtime_latest_result_json"));
      m_runtimeChannelSnapshotWindowJson =
          reinterpret_cast<RuntimeChannelSnapshotWindowResultJsonFn>(
              m_library.resolve("chaft_decrypted_workspace_channel_snapshot_"
                                "from_runtime_window_result_json"));
      m_storeSnapshotJson = reinterpret_cast<StoreSnapshotResultJsonFn>(
          m_library.resolve("chaft_workspace_snapshot_from_store_result_json"));
      m_storeSnapshotLatestJson =
          reinterpret_cast<StoreSnapshotLatestResultJsonFn>(m_library.resolve(
              "chaft_workspace_snapshot_from_store_latest_result_json"));
      m_storeSnapshotWindowJson =
          reinterpret_cast<StoreSnapshotWindowResultJsonFn>(m_library.resolve(
              "chaft_workspace_snapshot_from_store_window_result_json"));
      m_deviceIdJson = reinterpret_cast<RuntimeDeviceIdResultJsonFn>(
          m_library.resolve("chaft_runtime_device_id_result_json"));
      m_listWorkspacesJson =
          reinterpret_cast<RuntimeListWorkspacesResultJsonFn>(
              m_library.resolve("chaft_runtime_list_workspaces_result_json"));
      m_listWorkspacePageJson =
          reinterpret_cast<RuntimeListWorkspacePageResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_list_workspace_page_result_json"));
      m_listWorkspaceChannelPageJson =
          reinterpret_cast<RuntimeListWorkspaceChannelPageResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_list_workspace_channel_page_result_json"));
      m_listWorkspaceChannelPageContainingJson = reinterpret_cast<
          RuntimeListWorkspaceChannelPageContainingResultJsonFn>(
          m_library.resolve(
              "chaft_runtime_list_workspace_channel_page_containing_"
              "result_json"));
      m_listWorkspaceMemberPageJson =
          reinterpret_cast<RuntimeListWorkspaceMemberPageResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_list_workspace_member_page_result_json"));
      m_createWorkspaceJson =
          reinterpret_cast<RuntimeCreateWorkspaceResultJsonFn>(
              m_library.resolve("chaft_runtime_create_workspace_result_json"));
      m_createChannelJson = reinterpret_cast<RuntimeCreateChannelResultJsonFn>(
          m_library.resolve("chaft_runtime_create_channel_result_json"));
      m_updateDeviceProfileJson =
          reinterpret_cast<RuntimeUpdateDeviceProfileResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_update_device_profile_result_json"));
      m_publishDeviceKeyPackageJson =
          reinterpret_cast<RuntimePublishDeviceKeyPackageResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_publish_device_key_package_result_json"));
      m_publishPeerEndpointJson =
          reinterpret_cast<RuntimePublishPeerEndpointResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_publish_peer_endpoint_result_json"));
      m_publishPeerEndpointWithReplicaCapabilityJson = reinterpret_cast<
          RuntimePublishPeerEndpointWithReplicaCapabilityResultJsonFn>(
          m_library.resolve(
              "chaft_runtime_publish_peer_endpoint_with_replica_capability_"
              "result_json"));
      m_publishOpenMlsDeviceKeyPackageJson =
          reinterpret_cast<RuntimeOpenMlsWorkspaceActionResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_publish_openmls_device_key_package_result_"
                  "json"));
      m_createOpenMlsWorkspaceGroupJson =
          reinterpret_cast<RuntimeOpenMlsWorkspaceActionResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_create_openmls_workspace_group_result_json"));
      m_addOpenMlsWorkspaceGroupMemberJson =
          reinterpret_cast<RuntimeOpenMlsWorkspaceValueResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_add_openmls_workspace_group_member_result_"
                  "json"));
      m_joinOpenMlsWorkspaceGroupJson =
          reinterpret_cast<RuntimeOpenMlsWorkspaceValueResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_join_openmls_workspace_group_result_json"));
      m_updateOpenMlsWorkspaceGroupJson =
          reinterpret_cast<RuntimeOpenMlsWorkspaceActionResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_update_openmls_workspace_group_result_json"));
      m_updateWorkspaceOpenMlsGroupsJson =
          reinterpret_cast<RuntimeOpenMlsWorkspaceActionResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_update_workspace_openmls_groups_result_json"));
      m_applyOpenMlsWorkspaceGroupCommitsJson =
          reinterpret_cast<RuntimeOpenMlsWorkspaceValueResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_apply_openmls_workspace_group_commits_result_"
                  "json"));
      m_createOpenMlsChannelGroupJson =
          reinterpret_cast<RuntimeOpenMlsChannelActionResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_create_openmls_channel_group_result_json"));
      m_addOpenMlsChannelGroupMemberJson =
          reinterpret_cast<RuntimeOpenMlsChannelValueResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_add_openmls_channel_group_member_result_"
                  "json"));
      m_joinOpenMlsChannelGroupJson =
          reinterpret_cast<RuntimeOpenMlsChannelValueResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_join_openmls_channel_group_result_json"));
      m_updateOpenMlsChannelGroupJson =
          reinterpret_cast<RuntimeOpenMlsChannelActionResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_update_openmls_channel_group_result_json"));
      m_applyOpenMlsChannelGroupCommitsJson =
          reinterpret_cast<RuntimeOpenMlsChannelValueResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_apply_openmls_channel_group_commits_result_"
                  "json"));
      m_sendMessageJson = reinterpret_cast<RuntimeSendMessageResultJsonFn>(
          m_library.resolve("chaft_runtime_send_message_result_json"));
      m_sendMessageReplyJson =
          reinterpret_cast<RuntimeSendMessageReplyResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_send_message_reply_result_json"));
      m_sendAttachmentJson =
          reinterpret_cast<RuntimeSendAttachmentResultJsonFn>(
              m_library.resolve("chaft_runtime_send_attachment_result_json"));
      m_sendAttachmentReplyJson =
          reinterpret_cast<RuntimeSendAttachmentReplyResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_send_attachment_reply_result_json"));
      m_saveAttachmentJson =
          reinterpret_cast<RuntimeSaveAttachmentResultJsonFn>(
              m_library.resolve("chaft_runtime_save_attachment_result_json"));
      m_pruneBlobsJson = reinterpret_cast<RuntimePruneBlobsResultJsonFn>(
          m_library.resolve("chaft_runtime_prune_blobs_result_json"));
      m_editMessageJson = reinterpret_cast<RuntimeEditMessageResultJsonFn>(
          m_library.resolve("chaft_runtime_edit_message_result_json"));
      m_deleteMessageJson = reinterpret_cast<RuntimeDeleteMessageResultJsonFn>(
          m_library.resolve("chaft_runtime_delete_message_result_json"));
      m_addReactionJson = reinterpret_cast<RuntimeAddReactionResultJsonFn>(
          m_library.resolve("chaft_runtime_add_reaction_result_json"));
      m_removeReactionJson =
          reinterpret_cast<RuntimeRemoveReactionResultJsonFn>(
              m_library.resolve("chaft_runtime_remove_reaction_result_json"));
      m_markChannelReadJson =
          reinterpret_cast<RuntimeMarkChannelReadResultJsonFn>(
              m_library.resolve("chaft_runtime_mark_channel_read_result_json"));
      m_inviteMemberJson = reinterpret_cast<RuntimeInviteMemberResultJsonFn>(
          m_library.resolve("chaft_runtime_invite_member_result_json"));
      m_removeMemberWithOpenMlsJson =
          reinterpret_cast<RuntimeRemoveMemberResultJsonFn>(m_library.resolve(
              "chaft_runtime_remove_member_with_openmls_result_json"));
      m_removeMemberWithKeyRotationJson =
          reinterpret_cast<RuntimeRemoveMemberResultJsonFn>(m_library.resolve(
              "chaft_runtime_remove_member_with_key_rotation_result_json"));
      m_addChannelMemberJson =
          reinterpret_cast<RuntimeAddChannelMemberResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_add_channel_member_result_json"));
      m_removeChannelMemberWithOpenMlsJson =
          reinterpret_cast<RuntimeRemoveChannelMemberResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_remove_channel_member_with_openmls_result_"
                  "json"));
      m_removeChannelMemberWithKeyRotationJson =
          reinterpret_cast<RuntimeRemoveChannelMemberResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_remove_channel_member_with_key_rotation_"
                  "result_json"));
      m_exportWorkspaceKeyJson =
          reinterpret_cast<RuntimeExportWorkspaceKeyResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_export_workspace_key_result_json"));
      m_exportTrustSnapshotJson =
          reinterpret_cast<RuntimeExportTrustSnapshotResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_export_trust_snapshot_result_json"));
      m_rotateWorkspaceManualKeysJson =
          reinterpret_cast<RuntimeRotateWorkspaceManualKeysResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_rotate_workspace_manual_keys_result_json"));
      m_rotateWorkspaceForSuspectedCompromiseJson = reinterpret_cast<
          RuntimeRotateWorkspaceForSuspectedCompromiseResultJsonFn>(
          m_library.resolve(
              "chaft_runtime_rotate_workspace_for_suspected_compromise_"
              "result_json"));
      m_detectCompromiseJson =
          reinterpret_cast<RuntimeDetectCompromiseResultJsonFn>(
              m_library.resolve("chaft_runtime_detect_compromise_result_json"));
      m_respondCompromiseJson =
          reinterpret_cast<RuntimeOpenMlsWorkspaceActionResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_respond_compromise_result_json"));
      m_importWorkspaceKeyJson =
          reinterpret_cast<RuntimeImportWorkspaceKeyResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_import_workspace_key_result_json"));
      m_exportChannelKeyJson =
          reinterpret_cast<RuntimeExportChannelKeyResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_export_channel_key_result_json"));
      m_rotateChannelKeyJson =
          reinterpret_cast<RuntimeRotateChannelKeyResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_rotate_channel_key_result_json"));
      m_importChannelKeyJson =
          reinterpret_cast<RuntimeImportChannelKeyResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_import_channel_key_result_json"));
      m_exportRecoveryBundleJson =
          reinterpret_cast<RuntimeExportRecoveryBundleResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_export_recovery_bundle_result_json"));
      m_importRecoveryBundleJson =
          reinterpret_cast<RuntimeImportRecoveryBundleResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_import_recovery_bundle_result_json"));
      m_reindexWorkspaceSearchJson =
          reinterpret_cast<RuntimeReindexWorkspaceSearchResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_reindex_workspace_search_result_json"));
      m_searchWorkspaceJson =
          reinterpret_cast<RuntimeSearchWorkspaceResultJsonFn>(
              m_library.resolve("chaft_runtime_search_workspace_result_json"));
      m_searchWorkspaceChannelsJson =
          reinterpret_cast<RuntimeSearchWorkspaceChannelsResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_search_workspace_channels_result_json"));
      m_publishWorkspaceJson =
          reinterpret_cast<RuntimeDirectSyncResultJsonFn>(m_library.resolve(
              "chaft_runtime_publish_workspace_direct_result_json"));
      m_backupWorkspaceJson =
          reinterpret_cast<RuntimeDirectSyncResultJsonFn>(m_library.resolve(
              "chaft_runtime_backup_workspace_direct_result_json"));
      m_publishEventWithTrustSnapshotJson =
          reinterpret_cast<RuntimeDirectEventPublishResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_publish_event_with_trust_snapshot_direct_"
                  "result_json"));
      m_pullWorkspaceJson = reinterpret_cast<RuntimeDirectSyncResultJsonFn>(
          m_library.resolve("chaft_runtime_pull_workspace_direct_result_json"));
      m_syncWorkspaceJson = reinterpret_cast<RuntimeDirectSyncResultJsonFn>(
          m_library.resolve("chaft_runtime_sync_workspace_direct_result_json"));
      m_retryBlobTransfersJson =
          reinterpret_cast<RuntimeDirectRetryResultJsonFn>(m_library.resolve(
              "chaft_runtime_retry_blob_transfers_direct_result_json"));
      m_workspacePublishQueueJson =
          reinterpret_cast<RuntimeWorkspacePublishQueueResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_workspace_publish_queue_result_json"));
      m_workspaceStorageHealthJson =
          reinterpret_cast<RuntimeWorkspaceStorageHealthResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_workspace_storage_health_result_json"));
      m_repairWorkspaceStorageMetadataJson =
          reinterpret_cast<RuntimeRepairWorkspaceStorageMetadataResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_repair_workspace_storage_metadata_result_"
                  "json"));
      m_startDirectPeerJson =
          reinterpret_cast<RuntimeStartDirectPeerResultJsonFn>(
              m_library.resolve("chaft_runtime_start_direct_peer_result_json"));
      m_startIrohPeerJson = reinterpret_cast<RuntimeStartIrohPeerResultJsonFn>(
          m_library.resolve("chaft_runtime_start_iroh_peer_result_json"));
      m_stopDirectPeerJson =
          reinterpret_cast<RuntimeStopDirectPeerResultJsonFn>(
              m_library.resolve("chaft_runtime_stop_direct_peer_result_json"));

      m_ffiReady =
          m_freeString != nullptr &&
          (m_runtimeSnapshotJson != nullptr ||
           m_runtimeSnapshotLatestJson != nullptr) &&
          m_deviceIdJson != nullptr && m_createWorkspaceJson != nullptr &&
          (m_listWorkspacesJson != nullptr ||
           m_listWorkspacePageJson != nullptr) &&
          m_createChannelJson != nullptr && m_sendMessageJson != nullptr &&
          m_updateDeviceProfileJson != nullptr &&
          m_publishDeviceKeyPackageJson != nullptr &&
          m_sendAttachmentJson != nullptr && m_saveAttachmentJson != nullptr &&
          m_pruneBlobsJson != nullptr && m_editMessageJson != nullptr &&
          m_deleteMessageJson != nullptr && m_addReactionJson != nullptr &&
          m_removeReactionJson != nullptr && m_markChannelReadJson != nullptr &&
          m_inviteMemberJson != nullptr &&
          m_removeMemberWithOpenMlsJson != nullptr &&
          m_removeMemberWithKeyRotationJson != nullptr &&
          m_publishOpenMlsDeviceKeyPackageJson != nullptr &&
          m_createOpenMlsWorkspaceGroupJson != nullptr &&
          m_addOpenMlsWorkspaceGroupMemberJson != nullptr &&
          m_joinOpenMlsWorkspaceGroupJson != nullptr &&
          m_updateOpenMlsWorkspaceGroupJson != nullptr &&
          m_updateWorkspaceOpenMlsGroupsJson != nullptr &&
          m_applyOpenMlsWorkspaceGroupCommitsJson != nullptr &&
          m_createOpenMlsChannelGroupJson != nullptr &&
          m_addOpenMlsChannelGroupMemberJson != nullptr &&
          m_joinOpenMlsChannelGroupJson != nullptr &&
          m_updateOpenMlsChannelGroupJson != nullptr &&
          m_applyOpenMlsChannelGroupCommitsJson != nullptr &&
          m_addChannelMemberJson != nullptr &&
          m_removeChannelMemberWithOpenMlsJson != nullptr &&
          m_removeChannelMemberWithKeyRotationJson != nullptr &&
          m_exportWorkspaceKeyJson != nullptr &&
          m_exportTrustSnapshotJson != nullptr &&
          m_rotateWorkspaceManualKeysJson != nullptr &&
          m_rotateWorkspaceForSuspectedCompromiseJson != nullptr &&
          m_detectCompromiseJson != nullptr &&
          m_respondCompromiseJson != nullptr &&
          m_importWorkspaceKeyJson != nullptr &&
          m_exportChannelKeyJson != nullptr &&
          m_rotateChannelKeyJson != nullptr &&
          m_importChannelKeyJson != nullptr &&
          m_exportRecoveryBundleJson != nullptr &&
          m_importRecoveryBundleJson != nullptr &&
          m_reindexWorkspaceSearchJson != nullptr &&
          m_searchWorkspaceJson != nullptr &&
          m_publishWorkspaceJson != nullptr &&
          m_backupWorkspaceJson != nullptr &&
          m_publishEventWithTrustSnapshotJson != nullptr &&
          m_pullWorkspaceJson != nullptr && m_syncWorkspaceJson != nullptr &&
          m_retryBlobTransfersJson != nullptr &&
          m_startDirectPeerJson != nullptr && m_stopDirectPeerJson != nullptr;
      if (m_ffiReady) {
        return;
      }

      m_library.unload();
    }

    setSyncStatus(QStringLiteral("FFI library unavailable"));
  }

  bool ensureFfiReady() {
    if (m_ffiReady) {
      return true;
    }
    setSyncStatus(QStringLiteral("FFI library unavailable"));
    return false;
  }

  bool validateFfiPathForDispatch(const QString &path, const QString &label,
                                  bool allowEmpty) {
    if (path.isEmpty()) {
      if (allowEmpty) {
        return true;
      }
      setSyncStatus(QStringLiteral("%1 required").arg(label));
      return false;
    }

    QString metadataError;
    if (!validateMetadataTextForWrite(path, kMaxFfiPathBytes, label,
                                      QStringLiteral("64 KB"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    return true;
  }

  bool validateRuntimeDataDirForDispatch() {
    if (m_runtimeDir.isEmpty()) {
      setSyncStatus(QStringLiteral("runtime unavailable"));
      return false;
    }
    return validateFfiPathForDispatch(
        m_runtimeDir, QStringLiteral("runtime data directory"), false);
  }

  bool validateRuntimePathsForDispatch() {
    return validateRuntimeDataDirForDispatch() &&
           validateFfiPathForDispatch(
               m_identityFile, QStringLiteral("identity file path"), true);
  }

  bool validateRawEventStorePathForDispatch() {
    return validateFfiPathForDispatch(
        m_eventStorePath, QStringLiteral("event store path"), false);
  }

  bool ensureRuntimeAccessReady() {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_runtimeAccessSuspendedUntilUnlock) {
      setSyncStatus(QStringLiteral("runtime locked"));
      return false;
    }
    if (m_runtimeUnlockRequired) {
      setSyncStatus(QStringLiteral("passphrase required"));
      return false;
    }
    if (m_rawEventStoreMode) {
      setSyncStatus(QStringLiteral("runtime unavailable in event-store view"));
      return false;
    }
    return validateRuntimePathsForDispatch();
  }

  bool selectedWorkspaceIdForDispatch(QString *workspaceId, bool allowEmpty,
                                      const QString &emptyStatus) {
    const auto normalizedWorkspaceId = m_workspaceId.trimmed();
    if (m_workspaceId != normalizedWorkspaceId) {
      m_workspaceId = normalizedWorkspaceId;
      emit selectedWorkspaceChanged();
    }
    if (m_workspaceId.isEmpty()) {
      workspaceId->clear();
      if (!allowEmpty) {
        setSyncStatus(emptyStatus);
        return false;
      }
      return true;
    }

    QString metadataError;
    if (!validateMetadataTextForWrite(m_workspaceId, kMaxWorkspaceIdBytes,
                                      QStringLiteral("workspace ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    *workspaceId = m_workspaceId;
    return true;
  }

  bool ensureRuntimeWorkspace() {
    if (!ensureRuntimeAccessReady()) {
      return false;
    }
    if (m_runtimeDir.isEmpty()) {
      setSyncStatus(QStringLiteral("runtime unavailable"));
      return false;
    }

    QString workspaceId;
    if (!selectedWorkspaceIdForDispatch(
            &workspaceId, false, QStringLiteral("workspace ID required"))) {
      return false;
    }
    return true;
  }

  bool
  callWorkspaceOpenMlsAction(RuntimeOpenMlsWorkspaceActionResultJsonFn function,
                             const QString &unavailableStatus,
                             const QString &successStatus) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (function == nullptr) {
      setSyncStatus(unavailableStatus);
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("updating OpenMLS..."));
    runWorkspaceOpenMlsAction(function, successStatus, generation);
    return true;
  }

  bool callWorkspaceOpenMlsValueAction(
      RuntimeOpenMlsWorkspaceValueResultJsonFn function, const QString &value,
      bool allowEmptyValue, const QString &missingValueStatus,
      const QString &unavailableStatus, const QString &successStatus) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedValue = value.trimmed();
    if (!allowEmptyValue && normalizedValue.isEmpty()) {
      setSyncStatus(missingValueStatus);
      return false;
    }
    QString metadataError;
    if (!validateOpenMlsValueForWrite(normalizedValue, allowEmptyValue,
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (function == nullptr) {
      setSyncStatus(unavailableStatus);
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("updating OpenMLS..."));
    runWorkspaceOpenMlsValueAction(function, normalizedValue, allowEmptyValue,
                                   successStatus, generation);
    return true;
  }

  bool
  callChannelOpenMlsAction(RuntimeOpenMlsChannelActionResultJsonFn function,
                           const QString &channelId,
                           const QString &unavailableStatus,
                           const QString &successStatus) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedChannelId = channelId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("channel required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (function == nullptr) {
      setSyncStatus(unavailableStatus);
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("updating OpenMLS..."));
    runChannelOpenMlsAction(function, normalizedChannelId, successStatus,
                            generation);
    return true;
  }

  bool callChannelOpenMlsValueAction(
      RuntimeOpenMlsChannelValueResultJsonFn function, const QString &channelId,
      const QString &value, bool allowEmptyValue,
      const QString &missingValueStatus, const QString &unavailableStatus,
      const QString &successStatus) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedChannelId = channelId.trimmed();
    const auto normalizedValue = value.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("channel required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("channel ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!allowEmptyValue && normalizedValue.isEmpty()) {
      setSyncStatus(missingValueStatus);
      return false;
    }
    if (!validateOpenMlsValueForWrite(normalizedValue, allowEmptyValue,
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (function == nullptr) {
      setSyncStatus(unavailableStatus);
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("updating OpenMLS..."));
    runChannelOpenMlsValueAction(function, normalizedChannelId, normalizedValue,
                                 allowEmptyValue, successStatus, generation);
    return true;
  }

  void applyRuntimeSnapshot(const QJsonObject &value, bool updateStatus) {
    ++m_channelPageGeneration;
    ++m_memberPageGeneration;
    ++m_timelinePageGeneration;
    setChannelPageInFlight(false);
    setMemberPageInFlight(false);
    m_workspaceSnapshot = snapshotWithPreservedResolvedChannels(value);
    emit workspaceSnapshotChanged();
    if (!m_rawEventStoreMode) {
      queueWorkspaceSummariesRefresh();
      queuePublishQueueRefresh();
      queueWorkspaceStorageHealthRefresh();
      refreshActiveSearch();
    }
    if (updateStatus) {
      setSyncStatus(QStringLiteral("local event log"));
    }
  }

  void finishRuntimeWriteSnapshot(
      const QJsonObject &value, const QJsonObject &snapshotValue,
      const QString &error, const QString &snapshotError,
      const QString &workspaceId, quint64 generation,
      const QString &successStatus, const QString &workspaceSwitchStatus) {
    if (generation < m_lastAppliedRuntimeWriteGeneration) {
      queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(), workspaceId);
      return;
    }
    if (value.isEmpty()) {
      setSyncStatus(error);
      return;
    }
    if (snapshotValue.isEmpty()) {
      setSyncStatus(snapshotError);
      return;
    }
    if (m_workspaceId != workspaceId) {
      setSyncStatus(workspaceSwitchStatus);
      m_lastAppliedRuntimeWriteGeneration = generation;
      return;
    }

    applyRuntimeSnapshot(snapshotValue, false);
    m_lastAppliedRuntimeWriteGeneration = generation;
    setSyncStatus(successStatus);
  }

  QVariantMap
  snapshotWithPreservedResolvedChannels(const QJsonObject &value) const {
    auto snapshot = value.toVariantMap();
    const auto snapshotWorkspaceId =
        snapshot.value(QStringLiteral("workspaceId")).toString();
    const auto currentWorkspaceId =
        m_workspaceSnapshot.value(QStringLiteral("workspaceId")).toString();
    if (!snapshotWorkspaceId.isEmpty() &&
        snapshotWorkspaceId == currentWorkspaceId) {
      const auto resolvedChannels =
          m_workspaceSnapshot.value(QStringLiteral("resolvedChannels"));
      if (resolvedChannels.isValid() && !resolvedChannels.toMap().isEmpty()) {
        snapshot.insert(QStringLiteral("resolvedChannels"), resolvedChannels);
      }
    }
    return snapshot;
  }

  QString workspaceNameForId(const QString &workspaceId) const {
    for (const auto &summaryValue : m_workspaceSummaries) {
      const auto summary = summaryValue.toMap();
      if (summary.value(QStringLiteral("workspaceId")).toString() ==
          workspaceId) {
        const auto name = summary.value(QStringLiteral("name")).toString();
        if (!name.trimmed().isEmpty()) {
          return name;
        }
      }
    }
    return QStringLiteral("Chaft");
  }

  QVariantMap loadingWorkspaceSnapshot(const QString &workspaceId) const {
    QVariantMap timelineWindow;
    timelineWindow.insert(QStringLiteral("startIndex"), 0);
    timelineWindow.insert(QStringLiteral("itemCount"), 0);
    timelineWindow.insert(QStringLiteral("totalCount"), 0);
    timelineWindow.insert(QStringLiteral("hasMoreBefore"), false);
    timelineWindow.insert(QStringLiteral("hasMoreAfter"), false);

    QVariantMap snapshot;
    snapshot.insert(QStringLiteral("workspaceId"), workspaceId);
    snapshot.insert(QStringLiteral("name"), workspaceNameForId(workspaceId));
    snapshot.insert(QStringLiteral("channels"), QVariantList{});
    snapshot.insert(QStringLiteral("channelCount"), 0);
    snapshot.insert(QStringLiteral("profiles"), QVariantList{});
    snapshot.insert(QStringLiteral("members"), QVariantList{});
    snapshot.insert(QStringLiteral("keyPackages"), QVariantList{});
    snapshot.insert(QStringLiteral("timelineWindow"), timelineWindow);
    snapshot.insert(QStringLiteral("timeline"), QVariantList{});
    snapshot.insert(QStringLiteral("gapCount"), 0);
    snapshot.insert(QStringLiteral("gaps"), QVariantList{});
    snapshot.insert(QStringLiteral("invalidSignatureCount"), 0);
    snapshot.insert(QStringLiteral("invalidSignatures"), QVariantList{});
    return snapshot;
  }

  void applyWorkspaceLoadingSnapshot(const QString &workspaceId) {
    ++m_channelPageGeneration;
    ++m_memberPageGeneration;
    ++m_timelinePageGeneration;
    setChannelPageInFlight(false);
    setMemberPageInFlight(false);
    m_workspaceSnapshot = loadingWorkspaceSnapshot(workspaceId);
    emit workspaceSnapshotChanged();
    clearPublishQueue();
  }

  void prependTimelineWindow(const QVariantMap &pageSnapshot) {
    QVariantList mergedTimeline =
        pageSnapshot.value(QStringLiteral("timeline")).toList();
    const auto currentTimeline =
        m_workspaceSnapshot.value(QStringLiteral("timeline")).toList();
    for (const auto &item : currentTimeline) {
      mergedTimeline.append(item);
    }

    const auto pageWindow =
        pageSnapshot.value(QStringLiteral("timelineWindow")).toMap();
    const auto currentWindow =
        m_workspaceSnapshot.value(QStringLiteral("timelineWindow")).toMap();
    const auto startIndex =
        pageWindow.value(QStringLiteral("startIndex")).toULongLong();
    auto totalCount =
        pageWindow.value(QStringLiteral("totalCount")).toULongLong();
    if (totalCount == 0) {
      totalCount =
          currentWindow.value(QStringLiteral("totalCount")).toULongLong();
    }
    if (totalCount == 0) {
      totalCount = static_cast<qulonglong>(mergedTimeline.size());
    }
    const auto endIndex =
        startIndex + static_cast<qulonglong>(mergedTimeline.size());

    QVariantMap mergedWindow;
    mergedWindow.insert(QStringLiteral("startIndex"), startIndex);
    mergedWindow.insert(QStringLiteral("itemCount"), mergedTimeline.size());
    mergedWindow.insert(QStringLiteral("totalCount"), totalCount);
    mergedWindow.insert(QStringLiteral("hasMoreBefore"), startIndex > 0);
    mergedWindow.insert(QStringLiteral("hasMoreAfter"), endIndex < totalCount);

    m_workspaceSnapshot.insert(QStringLiteral("timeline"), mergedTimeline);
    m_workspaceSnapshot.insert(QStringLiteral("timelineWindow"), mergedWindow);
    emit workspaceSnapshotChanged();
  }

  QVariantMap timelineItemForEvent(const QString &eventId) const {
    const auto timeline =
        m_workspaceSnapshot.value(QStringLiteral("timeline")).toList();
    for (const auto &itemValue : timeline) {
      const auto item = itemValue.toMap();
      if (item.value(QStringLiteral("eventId")).toString() == eventId) {
        return item;
      }
    }
    return {};
  }

  bool mergeResolvedChannelsFromSearchHits(const QJsonArray &hits) {
    auto resolvedChannels =
        m_workspaceSnapshot.value(QStringLiteral("resolvedChannels")).toMap();
    bool changed = false;
    for (const auto &hitValue : hits) {
      const auto hit = hitValue.toObject();
      const auto channelId =
          hit.value(QStringLiteral("channelId")).toString().trimmed();
      if (channelId.isEmpty() || resolvedChannels.contains(channelId)) {
        continue;
      }

      QVariantMap row;
      row.insert(QStringLiteral("channelId"), channelId);
      const auto channelName =
          hit.value(QStringLiteral("channelName")).toString().trimmed();
      row.insert(QStringLiteral("name"), channelName.isEmpty()
                                             ? QStringLiteral("Loading")
                                             : channelName);
      row.insert(QStringLiteral("isPrivate"),
                 hit.value(QStringLiteral("channelIsPrivate")).toBool(false));
      row.insert(QStringLiteral("unreadCount"), 0);
      resolvedChannels.insert(channelId, row);
      changed = true;
    }

    if (changed) {
      m_workspaceSnapshot.insert(QStringLiteral("resolvedChannels"),
                                 resolvedChannels);
    }
    return changed;
  }

  void applyWorkspaceSearchResults(const QString &query,
                                   const QJsonObject &value) {
    QVariantList rows;
    const auto hits = value.value(QStringLiteral("hits")).toArray();
    rows.reserve(hits.size());
    for (const auto &hitValue : hits) {
      const auto hit = hitValue.toObject();
      const auto eventId = hit.value(QStringLiteral("eventId")).toString();
      const auto physicalMs = hit.value(QStringLiteral("physicalMs"));
      auto row = timelineItemForEvent(eventId);
      if (row.isEmpty()) {
        row.insert(QStringLiteral("kind"), QStringLiteral("message"));
        row.insert(QStringLiteral("eventId"), eventId);
        row.insert(QStringLiteral("messageId"),
                   hit.value(QStringLiteral("messageId")).toString());
        row.insert(QStringLiteral("authorDeviceId"),
                   hit.value(QStringLiteral("authorDeviceId")).toString());
        row.insert(QStringLiteral("authorDisplayName"),
                   hit.value(QStringLiteral("authorDisplayName")).toVariant());
        row.insert(QStringLiteral("attachmentCount"), 0);
        row.insert(QStringLiteral("attachments"), QVariantList{});
        row.insert(QStringLiteral("reactionCount"), 0);
        row.insert(QStringLiteral("reactions"), QVariantMap{});
        row.insert(QStringLiteral("myReactions"), QVariantList{});
        row.insert(QStringLiteral("encrypted"), true);
        row.insert(QStringLiteral("deleted"), false);
        row.insert(QStringLiteral("missingParentIds"), QVariantList{});
      } else if (!row.contains(QStringLiteral("messageId"))) {
        row.insert(QStringLiteral("messageId"),
                   hit.value(QStringLiteral("messageId")).toString());
      }
      if (!physicalMs.isUndefined() && !physicalMs.isNull()) {
        row.insert(QStringLiteral("physicalMs"), physicalMs.toVariant());
      }
      if (row.value(QStringLiteral("authorDeviceId")).toString().isEmpty()) {
        row.insert(QStringLiteral("authorDeviceId"),
                   hit.value(QStringLiteral("authorDeviceId")).toString());
      }
      if (row.value(QStringLiteral("authorDisplayName")).toString().isEmpty() &&
          !hit.value(QStringLiteral("authorDisplayName")).isUndefined() &&
          !hit.value(QStringLiteral("authorDisplayName")).isNull()) {
        row.insert(QStringLiteral("authorDisplayName"),
                   hit.value(QStringLiteral("authorDisplayName")).toVariant());
      }
      row.insert(QStringLiteral("channelId"),
                 hit.value(QStringLiteral("channelId")).toString());
      if (!hit.value(QStringLiteral("channelName")).isUndefined() &&
          !hit.value(QStringLiteral("channelName")).isNull()) {
        row.insert(QStringLiteral("channelName"),
                   hit.value(QStringLiteral("channelName")).toString());
      }
      if (!hit.value(QStringLiteral("channelIsPrivate")).isUndefined() &&
          !hit.value(QStringLiteral("channelIsPrivate")).isNull()) {
        row.insert(QStringLiteral("channelIsPrivate"),
                   hit.value(QStringLiteral("channelIsPrivate")).toBool());
      }
      const auto body = hit.value(QStringLiteral("body")).toString();
      row.insert(QStringLiteral("body"), body);
      row.insert(QStringLiteral("bodyCharCount"),
                 std::max(0, hit.value(QStringLiteral("bodyCharCount"))
                                 .toInt(body.size())));
      row.insert(QStringLiteral("bodyTruncated"),
                 hit.value(QStringLiteral("bodyTruncated")).toBool(false));
      row.insert(QStringLiteral("searchResult"), true);
      rows.append(row);
    }

    m_messageSearchQuery = query;
    m_messageSearchHits = rows;
    m_messageSearchHitCount =
        std::max(0, value.value(QStringLiteral("hitCount")).toInt(rows.size()));
    m_messageSearchHasMoreHits =
        value.value(QStringLiteral("hasMoreHits")).toBool(false);
    if (mergeResolvedChannelsFromSearchHits(hits)) {
      emit workspaceSnapshotChanged();
    }
    emit messageSearchChanged();
  }

  void refreshActiveSearch() {
    if (!m_messageSearchQuery.trimmed().isEmpty()) {
      searchWorkspaceMessages(m_messageSearchQuery);
    }
    if (!m_channelSearchQuery.trimmed().isEmpty()) {
      searchWorkspaceChannels(m_channelSearchQuery);
    }
  }

  void applyWorkspaceChannelSearchResults(const QString &query,
                                          const QJsonObject &value) {
    QVariantList rows;
    const auto channels = value.value(QStringLiteral("channels")).toArray();
    rows.reserve(channels.size());
    for (const auto &channelValue : channels) {
      rows.append(channelValue.toObject().toVariantMap());
    }

    const auto merged = mergeResolvedChannelRows(channels);
    m_channelSearchQuery = query;
    m_channelSearchResults = rows;
    if (merged > 0) {
      emit workspaceSnapshotChanged();
    }
    emit channelSearchChanged();
  }

  qsizetype mergeResolvedChannelRows(const QJsonArray &pageRows) {
    auto resolvedChannels =
        m_workspaceSnapshot.value(QStringLiteral("resolvedChannels")).toMap();
    auto loadedChannels =
        m_workspaceSnapshot.value(QStringLiteral("channels")).toList();
    qsizetype merged = 0;

    for (const auto &rowValue : pageRows) {
      const auto row = rowValue.toObject().toVariantMap();
      const auto channelId = row.value(QStringLiteral("channelId")).toString();
      if (channelId.isEmpty()) {
        continue;
      }

      resolvedChannels.insert(channelId, row);
      ++merged;
      for (qsizetype index = 0; index < loadedChannels.size(); ++index) {
        const auto loaded = loadedChannels.at(index).toMap();
        if (loaded.value(QStringLiteral("channelId")).toString() == channelId) {
          loadedChannels[index] = row;
          break;
        }
      }
    }

    if (merged > 0) {
      m_workspaceSnapshot.insert(QStringLiteral("channels"), loadedChannels);
      m_workspaceSnapshot.insert(QStringLiteral("resolvedChannels"),
                                 resolvedChannels);
    }
    return merged;
  }

  void queueRuntimeSnapshotRefresh(bool updateStatus = false) {
    if (!validateRuntimePathsForDispatch()) {
      return;
    }
    QString workspaceId;
    if (!selectedWorkspaceIdForDispatch(
            &workspaceId, false, QStringLiteral("workspace ID required"))) {
      return;
    }
    const auto generation = ++m_runtimeWriteGeneration;
    runRuntimeSnapshotRefresh(generation, updateStatus);
  }

  void queueRuntimeSnapshotRefreshIfCurrent(bool shouldRefresh,
                                            const QString &workspaceId) {
    if (!shouldRefresh || m_workspaceId != workspaceId ||
        m_runtimeAccessSuspendedUntilUnlock || m_runtimeUnlockRequired) {
      return;
    }

    queueRuntimeSnapshotRefresh();
  }

  void queueWorkspaceSummariesRefresh() {
    if (!m_ffiReady ||
        (m_listWorkspacesJson == nullptr &&
         m_listWorkspacePageJson == nullptr) ||
        m_freeString == nullptr) {
      return;
    }
    if (!validateRuntimePathsForDispatch()) {
      return;
    }

    const auto generation = ++m_workspaceSummariesGeneration;
    runWorkspaceSummariesRefresh(generation);
  }

  void queuePublishQueueRefresh() {
    if (!hasRuntimeWorkspace() || m_workspacePublishQueueJson == nullptr ||
        m_freeString == nullptr) {
      clearPublishQueue();
      return;
    }

    QString workspaceId;
    if (!selectedWorkspaceIdForDispatch(
            &workspaceId, false, QStringLiteral("workspace ID required"))) {
      clearPublishQueue();
      return;
    }
    const auto generation = ++m_publishQueueGeneration;
    runPublishQueueRefresh(generation, workspaceId);
  }

  void queueWorkspaceStorageHealthRefresh() {
    if (!hasRuntimeWorkspace() || m_workspaceStorageHealthJson == nullptr ||
        m_freeString == nullptr) {
      clearWorkspaceStorageHealth();
      return;
    }

    QString workspaceId;
    if (!selectedWorkspaceIdForDispatch(
            &workspaceId, false, QStringLiteral("workspace ID required"))) {
      clearWorkspaceStorageHealth();
      return;
    }
    const auto generation = ++m_workspaceStorageHealthGeneration;
    runWorkspaceStorageHealthRefresh(generation, workspaceId);
  }

  void queueRuntimeHydration() {
    if (!m_ffiReady || m_freeString == nullptr) {
      return;
    }
    if (!validateRuntimePathsForDispatch()) {
      return;
    }
    QString workspaceId;
    if (!selectedWorkspaceIdForDispatch(&workspaceId, true, QString())) {
      return;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    const auto summariesGeneration = ++m_workspaceSummariesGeneration;
    runRuntimeHydration(generation, summariesGeneration, workspaceId);
  }

  void queueStoreSnapshotHydration() {
    if (!m_ffiReady || m_freeString == nullptr) {
      return;
    }
    if (!validateRawEventStorePathForDispatch()) {
      return;
    }
    QString workspaceId;
    if (!selectedWorkspaceIdForDispatch(
            &workspaceId, false, QStringLiteral("workspace ID required"))) {
      return;
    }
    if (m_storeSnapshotJson == nullptr &&
        m_storeSnapshotLatestJson == nullptr) {
      setSyncStatus(QStringLiteral("event store snapshot unavailable"));
      return;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    runStoreSnapshotHydration(generation, workspaceId);
  }

  void persistDesktopConfig() {
    saveDesktopConfig(m_runtimeDir, m_workspaceId, m_defaultPeerEndpoint,
                      m_backupPeerEndpoints, m_backupPeerStatuses,
                      m_autoBackupEnabled);
  }

  bool backupPeerInCooldown(const QString &peerEndpoint,
                            const QDateTime &now) const {
    const auto status = m_backupPeerStatuses.value(peerEndpoint).toMap();
    return backupPeerStatusInCooldown(status, now);
  }

  void recordBackupAttempt(const QString &peerEndpoint) {
    if (!m_backupPeerEndpoints.contains(peerEndpoint)) {
      return;
    }

    auto status = m_backupPeerStatuses.value(peerEndpoint).toMap();
    status.insert(QStringLiteral("lastAttemptAt"), currentUtcTimestamp());
    status.remove(QStringLiteral("nextAttemptAfter"));
    setBackupPeerStatus(peerEndpoint, status);
  }

  void recordBackupResult(const QString &peerEndpoint, bool success,
                          const QString &message, int missingBlobCount = 0,
                          int skippedGapCount = 0, bool suspectPeer = false) {
    if (!m_backupPeerEndpoints.contains(peerEndpoint)) {
      return;
    }

    auto status = m_backupPeerStatuses.value(peerEndpoint).toMap();
    const auto now = QDateTime::currentDateTimeUtc();
    const auto nowTimestamp = now.toString(Qt::ISODateWithMs);
    status.insert(QStringLiteral("lastMessage"), message);
    if (success) {
      const auto remainingSuspectScore =
          qMax(0, backupPeerSuspectScore(status) - 1);
      status.insert(QStringLiteral("failureCount"), 0);
      status.insert(QStringLiteral("lastSuccessAt"), nowTimestamp);
      status.insert(QStringLiteral("lastPartial"),
                    missingBlobCount > 0 || skippedGapCount > 0);
      if (remainingSuspectScore > 0) {
        status.insert(QStringLiteral("suspectScore"), remainingSuspectScore);
        status.insert(QStringLiteral("lastSuspectPeer"), true);
      } else {
        status.remove(QStringLiteral("suspectScore"));
        status.remove(QStringLiteral("lastSuspectPeer"));
        status.remove(QStringLiteral("lastSuspectAt"));
      }
      if (missingBlobCount > 0) {
        status.insert(QStringLiteral("lastMissingBlobCount"), missingBlobCount);
      } else {
        status.remove(QStringLiteral("lastMissingBlobCount"));
      }
      if (skippedGapCount > 0) {
        status.insert(QStringLiteral("lastSkippedGapCount"), skippedGapCount);
      } else {
        status.remove(QStringLiteral("lastSkippedGapCount"));
      }
      status.remove(QStringLiteral("nextAttemptAfter"));
    } else {
      const auto failureCount =
          status.value(QStringLiteral("failureCount")).toInt(0) + 1;
      status.insert(QStringLiteral("failureCount"), failureCount);
      status.insert(QStringLiteral("lastFailureAt"), nowTimestamp);
      status.insert(QStringLiteral("lastPartial"), false);
      status.remove(QStringLiteral("lastMissingBlobCount"));
      status.remove(QStringLiteral("lastSkippedGapCount"));
      if (suspectPeer) {
        status.insert(QStringLiteral("suspectScore"),
                      qMin(8, backupPeerSuspectScore(status) + 1));
        status.insert(QStringLiteral("lastSuspectPeer"), true);
        status.insert(QStringLiteral("lastSuspectAt"), nowTimestamp);
      }
      status.insert(QStringLiteral("nextAttemptAfter"),
                    now.addSecs(backupPeerCooldownSeconds(failureCount))
                        .toString(Qt::ISODateWithMs));
    }
    setBackupPeerStatus(peerEndpoint, status);
  }

  void reconcileBackupPeerPartialStateFromRetry(const QJsonObject &value) {
    auto statuses = m_backupPeerStatuses;
    QStringList seenBlobPeerPairs;
    auto changed = false;
    const auto repairedAt = currentUtcTimestamp();
    const auto attempts =
        value.value(QStringLiteral("blobTransferAttempts")).toArray();

    for (const auto &attemptValue : attempts) {
      const auto attempt = attemptValue.toObject();
      if (attempt.value(QStringLiteral("status")).toString() !=
          QStringLiteral("succeeded")) {
        continue;
      }

      const auto peerEndpoint =
          attempt.value(QStringLiteral("peerEndpoint")).toString();
      if (!m_backupPeerEndpoints.contains(peerEndpoint)) {
        continue;
      }

      const auto blobHash =
          attempt.value(QStringLiteral("blobHash")).toString();
      if (blobHash.isEmpty()) {
        continue;
      }

      const auto seenKey = peerEndpoint + QLatin1Char('\n') + blobHash;
      if (seenBlobPeerPairs.contains(seenKey)) {
        continue;
      }
      seenBlobPeerPairs.append(seenKey);

      auto status = statuses.value(peerEndpoint).toMap();
      if (!variantBoolValue(status.value(QStringLiteral("lastPartial")))) {
        continue;
      }

      auto remaining = qMax(
          1, status.value(QStringLiteral("lastMissingBlobCount")).toInt(0));
      const auto skippedGapCount =
          status.value(QStringLiteral("lastSkippedGapCount")).toInt(0);
      remaining = qMax(0, remaining - 1);
      if (remaining == 0) {
        status.remove(QStringLiteral("lastMissingBlobCount"));
        status.insert(QStringLiteral("lastRepairAt"), repairedAt);
        if (skippedGapCount > 0) {
          status.insert(QStringLiteral("lastPartial"), true);
          status.insert(QStringLiteral("lastMessage"),
                        QStringLiteral("backup partial, %1 skipped gap(s)")
                            .arg(skippedGapCount));
        } else {
          status.insert(QStringLiteral("lastPartial"), false);
          status.insert(QStringLiteral("lastMessage"),
                        QStringLiteral("backup blobs repaired"));
        }
      } else {
        status.insert(QStringLiteral("lastMissingBlobCount"), remaining);
        const auto message =
            skippedGapCount > 0
                ? QStringLiteral(
                      "backup partial, %1 missing blob(s), %2 skipped gap(s)")
                      .arg(remaining)
                      .arg(skippedGapCount)
                : QStringLiteral("backup partial, %1 missing blob(s)")
                      .arg(remaining);
        status.insert(QStringLiteral("lastMessage"), message);
      }
      statuses.insert(peerEndpoint, status);
      changed = true;
    }

    if (!changed) {
      return;
    }

    m_backupPeerStatuses =
        pruneBackupPeerStatuses(statuses, m_backupPeerEndpoints);
    persistDesktopConfig();
    emit backupPeerStatusesChanged();
  }

  void recordBackupPeerErrorsFromRetry(const QJsonObject &value) {
    QVariantMap peerFailures;
    const auto peerErrors = value.value(QStringLiteral("peerErrors")).toArray();
    for (const auto &peerErrorValue : peerErrors) {
      const auto peerError = peerErrorValue.toObject();
      const auto peerEndpoint =
          peerError.value(QStringLiteral("peerEndpoint")).toString();
      if (!m_backupPeerEndpoints.contains(peerEndpoint)) {
        continue;
      }

      auto failure = peerFailures.value(peerEndpoint).toMap();
      const auto existingSuspect =
          variantBoolValue(
              failure.value(QStringLiteral("suspectProtocolError")));
      failure.insert(QStringLiteral("suspectProtocolError"),
                     existingSuspect ||
                         peerError.value(QStringLiteral("suspectProtocolError"))
                             .toBool(false));
      if (!failure.contains(QStringLiteral("message"))) {
        const auto message = peerError.value(QStringLiteral("message"))
                                 .toString(QStringLiteral("blob retry failed"));
        failure.insert(QStringLiteral("message"), message);
      }
      peerFailures.insert(peerEndpoint, failure);
    }

    for (auto it = peerFailures.cbegin(); it != peerFailures.cend(); ++it) {
      const auto failure = it.value().toMap();
      recordBackupResult(
          it.key(), false,
          variantStringValue(failure.value(QStringLiteral("message")),
                             QStringLiteral("blob retry failed")),
          0, 0,
          variantBoolValue(
              failure.value(QStringLiteral("suspectProtocolError"))));
    }
  }

  void setBackupPeerStatus(const QString &peerEndpoint,
                           const QVariantMap &status) {
    m_backupPeerStatuses.insert(peerEndpoint, status);
    m_backupPeerStatuses =
        pruneBackupPeerStatuses(m_backupPeerStatuses, m_backupPeerEndpoints);
    persistDesktopConfig();
    emit backupPeerStatusesChanged();
  }

  bool startBackupWorkspace(const QString &peerEndpoint, bool persistDefault) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("sync already running"));
      return false;
    }
    if (m_backupWorkspaceJson == nullptr) {
      setSyncStatus(QStringLiteral("backup unavailable"));
      return false;
    }
    const auto endpoint = peerEndpoint.trimmed();
    if (endpoint.isEmpty()) {
      setSyncStatus(QStringLiteral("peer endpoint required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointForUse(endpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    if (persistDefault) {
      setDefaultPeerEndpoint(endpoint);
    }
    recordBackupAttempt(endpoint);
    setSyncStatus(QStringLiteral("backing up..."));
    runDirectSync(m_backupWorkspaceJson, endpoint, DirectSyncMode::Backup);
    return true;
  }

  void runDirectSync(RuntimeDirectSyncResultJsonFn syncFn,
                     const QString &peerEndpoint, DirectSyncMode mode) {
    setSyncInFlight(true);
    const auto generation =
        (mode == DirectSyncMode::Pull || mode == DirectSyncMode::Sync)
            ? ++m_runtimeWriteGeneration
            : m_runtimeWriteGeneration;
    const QPointer<ChaftController> guard(this);
    const auto freeString = m_freeString;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, syncFn, freeString, snapshotFn,
                                    snapshotLatestFn, runtimeDir, identityFile,
                                    workspaceId, peerEndpoint, mode, generation,
                                    timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto endpointBytes = peerEndpoint.toUtf8();
      char *raw = syncFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), endpointBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto errorCode = error.isEmpty() ? resultErrorCodeFromJson(json)
                                             : QStringLiteral("ffi_error");
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      const auto publishedValue =
          mode == DirectSyncMode::Sync
              ? value.value(QStringLiteral("published")).toObject()
              : value;
      const auto pulledValue =
          mode == DirectSyncMode::Sync
              ? value.value(QStringLiteral("pulled")).toObject()
              : value;
      const auto publishedCount = jsonCountOrArraySize(
          publishedValue, QStringLiteral("publishedEventCount"),
          QStringLiteral("publishedEventIds"));
      const auto publishedBlobCount = jsonCountOrArraySize(
          publishedValue, QStringLiteral("publishedBlobCount"),
          QStringLiteral("publishedBlobHashes"));
      const auto publishedMissingBlobCount = jsonCountOrArraySize(
          publishedValue, QStringLiteral("missingBlobCount"),
          QStringLiteral("missingBlobHashes"));
      const auto publishedSkippedGapCount = jsonCountOrArraySize(
          publishedValue, QStringLiteral("skippedGapCount"),
          QStringLiteral("skippedGaps"));
      const auto fetchedCount =
          jsonCountOrArraySize(pulledValue, QStringLiteral("fetchedEventCount"),
                               QStringLiteral("fetchedEventIds"));
      const auto fetchedBlobCount =
          jsonCountOrArraySize(pulledValue, QStringLiteral("fetchedBlobCount"),
                               QStringLiteral("fetchedBlobHashes"));
      const auto pulledMissingBlobCount =
          jsonCountOrArraySize(pulledValue, QStringLiteral("missingBlobCount"),
                               QStringLiteral("missingBlobHashes"));
      const auto pulledGapCount = jsonCountOrArraySize(
          pulledValue, QStringLiteral("gapCount"), QStringLiteral("gaps"));
      const auto openMlsCatchup =
          pulledValue.value(QStringLiteral("openmlsCatchup")).toObject();
      const auto openMlsCatchupCount =
          openMlsCatchupEventCountFromJson(openMlsCatchup);
      const auto compromiseSummary = compromiseResponseSummaryText(
          pulledValue.value(QStringLiteral("compromiseResponse")));
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty() &&
          (mode == DirectSyncMode::Pull || mode == DirectSyncMode::Sync)) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, publishedCount, publishedBlobCount,
           publishedMissingBlobCount, publishedSkippedGapCount, fetchedCount,
           fetchedBlobCount, pulledMissingBlobCount, pulledGapCount,
           openMlsCatchupCount, compromiseSummary, error, errorCode,
           snapshotError, mode, peerEndpoint, workspaceId, generation]() {
            if (guard.isNull()) {
              return;
            }

            guard->setSyncInFlight(false);
            if (value.isEmpty()) {
              if (mode == DirectSyncMode::Backup) {
                guard->recordBackupResult(peerEndpoint, false, error, 0, 0,
                                          isPeerProtocolFailureCode(errorCode));
              }
              guard->setSyncStatus(error);
              return;
            }

            if (mode == DirectSyncMode::Pull || mode == DirectSyncMode::Sync) {
              if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
                guard->queueRuntimeSnapshotRefreshIfCurrent(true, workspaceId);
              } else if (snapshotValue.isEmpty()) {
                guard->setSyncStatus(snapshotError);
                return;
              } else if (guard->m_workspaceId != workspaceId) {
                guard->setSyncStatus(
                    QStringLiteral("sync completed after workspace switch"));
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                return;
              } else {
                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
              }
            }
            if (mode == DirectSyncMode::Publish) {
              guard->setSyncStatus(
                  QStringLiteral(
                      "published %1 event(s), %2 blob(s), %3 missing blob(s), "
                      "%4 skipped gap(s)")
                      .arg(publishedCount)
                      .arg(publishedBlobCount)
                      .arg(publishedMissingBlobCount)
                      .arg(publishedSkippedGapCount));
            } else if (mode == DirectSyncMode::Backup) {
              const auto message =
                  QStringLiteral(
                      "backed up %1 event(s), %2 blob(s), %3 missing blob(s), "
                      "%4 skipped gap(s)")
                      .arg(publishedCount)
                      .arg(publishedBlobCount)
                      .arg(publishedMissingBlobCount)
                      .arg(publishedSkippedGapCount);
              guard->recordBackupResult(
                  peerEndpoint, true, message,
                  static_cast<int>(publishedMissingBlobCount),
                  static_cast<int>(publishedSkippedGapCount));
              guard->setSyncStatus(message);
            } else if (mode == DirectSyncMode::Pull) {
              auto message =
                  QStringLiteral("pulled %1 event(s), %2 blob(s), %3 missing "
                                 "blob(s), %4 gap(s), %5 MLS event(s)")
                      .arg(fetchedCount)
                      .arg(fetchedBlobCount)
                      .arg(pulledMissingBlobCount)
                      .arg(pulledGapCount)
                      .arg(openMlsCatchupCount);
              if (!compromiseSummary.isEmpty()) {
                message += QStringLiteral(", ") + compromiseSummary;
              }
              guard->setSyncStatus(message);
            } else {
              auto message =
                  QStringLiteral("synced %1 event(s), %2 blob(s), %3 missing "
                                 "blob(s), %4 skipped gap(s) up / %5 event(s), "
                                 "%6 blob(s), %7 missing blob(s), %8 gap(s), "
                                 "%9 MLS event(s) down")
                      .arg(publishedCount)
                      .arg(publishedBlobCount)
                      .arg(publishedMissingBlobCount)
                      .arg(publishedSkippedGapCount)
                      .arg(fetchedCount)
                      .arg(fetchedBlobCount)
                      .arg(pulledMissingBlobCount)
                      .arg(pulledGapCount)
                      .arg(openMlsCatchupCount);
              if (!compromiseSummary.isEmpty()) {
                message += QStringLiteral(", ") + compromiseSummary;
              }
              guard->setSyncStatus(message);
            }
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runBlobTransferRetry(const QStringList &peerEndpoints) {
    setSyncInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto freeString = m_freeString;
    const auto retryFn = m_retryBlobTransfersJson;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto peerEndpointsText = joinedPeerEndpoints(peerEndpoints);
    auto *thread = QThread::create([guard, retryFn, freeString, runtimeDir,
                                    identityFile, workspaceId,
                                    peerEndpointsText]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto peerEndpointsBytes = peerEndpointsText.toUtf8();
      char *raw = retryFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), peerEndpointsBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      const auto pendingCount =
          jsonCountOrArraySize(value, QStringLiteral("pendingAttemptCount"),
                               QStringLiteral("pendingAttemptIds"));
      const auto retriedCount =
          jsonCountOrArraySize(value, QStringLiteral("retriedBlobCount"),
                               QStringLiteral("retriedBlobHashes"));
      const auto reconciledCount =
          jsonCountOrArraySize(value, QStringLiteral("reconciledBlobCount"),
                               QStringLiteral("reconciledBlobHashes"));
      const auto missingCount =
          jsonCountOrArraySize(value, QStringLiteral("missingBlobCount"),
                               QStringLiteral("missingBlobHashes"));
      const auto peerErrorCount =
          jsonCountOrArraySize(value, QStringLiteral("peerErrorCount"),
                               QStringLiteral("peerErrors"));
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, pendingCount, retriedCount, reconciledCount,
           missingCount, peerErrorCount, error]() {
            if (guard.isNull()) {
              return;
            }

            guard->setSyncInFlight(false);
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            if (pendingCount == 0) {
              guard->setSyncStatus(QStringLiteral("no pending blob transfers"));
              return;
            }

            guard->recordBackupPeerErrorsFromRetry(value);
            guard->reconcileBackupPeerPartialStateFromRetry(value);
            guard->setSyncStatus(
                QStringLiteral("retried %1 blob(s), reconciled %2, missing %3, "
                               "%4 peer error(s)")
                    .arg(retriedCount)
                    .arg(reconciledCount)
                    .arg(missingCount)
                    .arg(peerErrorCount));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runBlobPrune() {
    setSyncInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto pruneFn = m_pruneBlobsJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    auto *thread = QThread::create([guard, pruneFn, freeString, runtimeDir,
                                    identityFile]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      char *raw =
          pruneFn(runtimeDirBytes.constData(),
                  identityFileBytes.isEmpty() ? nullptr
                                              : identityFileBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      const auto removedCount =
          jsonCountOrArraySize(value, QStringLiteral("removedBlobCount"),
                               QStringLiteral("removedBlobHashes")) +
          jsonCountOrArraySize(value, QStringLiteral("removedManifestCount"),
                               QStringLiteral("removedManifestHashes")) +
          jsonCountOrArraySize(value, QStringLiteral("removedChunkCount"),
                               QStringLiteral("removedChunkHashes")) +
          jsonCountOrArraySize(value, QStringLiteral("removedTempFileCount"),
                               QStringLiteral("removedTempFilePaths"));
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, removedCount, error]() {
            if (guard.isNull()) {
              return;
            }

            guard->setSyncInFlight(false);
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            guard->setSyncStatus(
                QStringLiteral("pruned %1 blob object(s)").arg(removedCount));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceSummariesRefresh(quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto listFn = m_listWorkspacesJson;
    const auto listPageFn = m_listWorkspacePageJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    auto *thread = QThread::create([guard, listFn, listPageFn, freeString,
                                    runtimeDir, identityFile, generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      QString error;
      const auto value = workspaceSummariesFromRuntime(
          listFn, listPageFn, freeString, runtimeDirBytes, identityFileBytes,
          &error);
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, generation]() {
            if (guard.isNull() ||
                guard->m_workspaceSummariesGeneration != generation) {
              return;
            }
            if (!error.isEmpty()) {
              if (guard->m_workspaceSummaries.isEmpty()) {
                guard->setSyncStatus(error);
              }
              return;
            }
            if (value != guard->m_workspaceSummaries) {
              guard->m_workspaceSummaries = value;
              emit guard->workspaceSummariesChanged();
            }
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceChannelPageLoad(std::size_t startIndex, std::size_t limit,
                                   const QString &workspaceId,
                                   quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto pageFn = m_listWorkspaceChannelPageJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    auto *thread = QThread::create([guard, pageFn, freeString, runtimeDir,
                                    identityFile, workspaceId, startIndex,
                                    limit, generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      char *raw = pageFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), startIndex, limit);

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, generation]() {
            if (guard.isNull() ||
                guard->m_channelPageGeneration != generation) {
              return;
            }

            guard->setChannelPageInFlight(false);
            if (guard->m_workspaceId != workspaceId) {
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            const auto rowsValue = value.value(QStringLiteral("channels"));
            if (!rowsValue.isArray()) {
              guard->setSyncStatus(
                  QStringLiteral("channel page did not contain channel rows"));
              return;
            }

            const auto pageRows = rowsValue.toArray();
            const auto pageStart = value.value(QStringLiteral("startIndex"))
                                       .toVariant()
                                       .toULongLong();
            auto channels =
                guard->m_workspaceSnapshot.value(QStringLiteral("channels"))
                    .toList();
            if (pageStart > static_cast<qulonglong>(channels.size())) {
              guard->setSyncStatus(QStringLiteral("channel page is stale"));
              return;
            }

            const auto overlap = static_cast<qsizetype>(channels.size()) -
                                 static_cast<qsizetype>(pageStart);
            qsizetype appended = 0;
            for (qsizetype index = std::max<qsizetype>(0, overlap);
                 index < pageRows.size(); ++index) {
              channels.push_back(pageRows.at(index).toVariant());
              ++appended;
            }

            guard->m_workspaceSnapshot.insert(QStringLiteral("channels"),
                                              channels);
            guard->m_workspaceSnapshot.insert(
                QStringLiteral("channelCount"),
                value.value(QStringLiteral("totalCount")).toVariant());
            emit guard->workspaceSnapshotChanged();

            if (appended > 0) {
              guard->setSyncStatus(
                  QStringLiteral("loaded %1 channel(s)").arg(appended));
            } else {
              guard->setSyncStatus(QStringLiteral("all channels loaded"));
            }
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceChannelPageContainingLoad(const QString &channelId,
                                             std::size_t limit,
                                             const QString &workspaceId,
                                             quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto pageFn = m_listWorkspaceChannelPageContainingJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    auto *thread = QThread::create([guard, pageFn, freeString, runtimeDir,
                                    identityFile, workspaceId, channelId, limit,
                                    generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto channelIdBytes = channelId.toUtf8();
      char *raw = pageFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), channelIdBytes.constData(), limit);

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, channelId, generation]() {
            if (guard.isNull() ||
                guard->m_channelPageGeneration != generation) {
              return;
            }

            guard->setChannelPageInFlight(false);
            if (guard->m_workspaceId != workspaceId) {
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            const auto rowsValue = value.value(QStringLiteral("channels"));
            if (!rowsValue.isArray()) {
              guard->setSyncStatus(
                  QStringLiteral("channel lookup did not contain rows"));
              return;
            }

            const auto pageRows = rowsValue.toArray();
            const auto merged = guard->mergeResolvedChannelRows(pageRows);
            guard->m_workspaceSnapshot.insert(
                QStringLiteral("channelCount"),
                value.value(QStringLiteral("totalCount")).toVariant());
            emit guard->workspaceSnapshotChanged();

            if (merged > 0) {
              guard->setSyncStatus(
                  QStringLiteral("loaded channel %1").arg(channelId));
            } else {
              guard->setSyncStatus(QStringLiteral("channel lookup was empty"));
            }
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceMemberPageLoad(std::size_t startIndex, std::size_t limit,
                                  const QString &workspaceId,
                                  quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto pageFn = m_listWorkspaceMemberPageJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    auto *thread = QThread::create([guard, pageFn, freeString, runtimeDir,
                                    identityFile, workspaceId, startIndex,
                                    limit, generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      char *raw = pageFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), startIndex, limit);

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, generation]() {
            if (guard.isNull() || guard->m_memberPageGeneration != generation) {
              return;
            }

            guard->setMemberPageInFlight(false);
            if (guard->m_workspaceId != workspaceId) {
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            const auto rowsValue = value.value(QStringLiteral("members"));
            if (!rowsValue.isArray()) {
              guard->setSyncStatus(
                  QStringLiteral("member page did not contain member rows"));
              return;
            }

            const auto pageRows = rowsValue.toArray();
            const auto pageStart = value.value(QStringLiteral("startIndex"))
                                       .toVariant()
                                       .toULongLong();
            auto members =
                guard->m_workspaceSnapshot.value(QStringLiteral("members"))
                    .toList();
            if (pageStart > static_cast<qulonglong>(members.size())) {
              guard->setSyncStatus(QStringLiteral("member page is stale"));
              return;
            }

            const auto overlap = static_cast<qsizetype>(members.size()) -
                                 static_cast<qsizetype>(pageStart);
            qsizetype appended = 0;
            for (qsizetype index = std::max<qsizetype>(0, overlap);
                 index < pageRows.size(); ++index) {
              members.push_back(pageRows.at(index).toVariant());
              ++appended;
            }

            guard->m_workspaceSnapshot.insert(QStringLiteral("members"),
                                              members);
            guard->m_workspaceSnapshot.insert(
                QStringLiteral("memberCount"),
                value.value(QStringLiteral("totalCount")).toVariant());
            emit guard->workspaceSnapshotChanged();

            if (appended > 0) {
              guard->setSyncStatus(
                  QStringLiteral("loaded %1 member(s)").arg(appended));
            } else {
              guard->setSyncStatus(QStringLiteral("all members loaded"));
            }
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runRuntimeHydration(quint64 generation, quint64 summariesGeneration,
                           const QString &requestedWorkspaceId) {
    const QPointer<ChaftController> guard(this);
    const auto deviceFn = m_deviceIdJson;
    const auto listFn = m_listWorkspacesJson;
    const auto listPageFn = m_listWorkspacePageJson;
    const auto respondCompromiseFn = m_respondCompromiseJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, deviceFn, listFn, listPageFn,
                                    respondCompromiseFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, requestedWorkspaceId,
                                    generation, summariesGeneration,
                                    timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();

      QJsonObject deviceValue;
      QString deviceError;
      if (deviceFn != nullptr) {
        char *raw = deviceFn(runtimeDirBytes.constData(),
                             identityFileBytes.isEmpty()
                                 ? nullptr
                                 : identityFileBytes.constData());
        const auto json = takeFfiString(raw, freeString, &deviceError);
        if (deviceError.isEmpty()) {
          deviceValue = resultValueFromJson(json, &deviceError);
        }
      }

      QVariantList summaries;
      QString summariesError;
      summaries = workspaceSummariesFromRuntime(
          listFn, listPageFn, freeString, runtimeDirBytes, identityFileBytes,
          &summariesError);

      auto selectedWorkspaceId = requestedWorkspaceId;
      if (selectedWorkspaceId.isEmpty() && !summaries.isEmpty()) {
        selectedWorkspaceId = summaries.first()
                                  .toMap()
                                  .value(QStringLiteral("workspaceId"))
                                  .toString();
      }

      QJsonObject snapshotValue;
      QString snapshotError;
      QString compromiseSummary;
      QString compromiseError;
      if (!selectedWorkspaceId.isEmpty()) {
        const auto workspaceIdBytes = selectedWorkspaceId.toUtf8();
        if (respondCompromiseFn != nullptr) {
          char *raw = respondCompromiseFn(runtimeDirBytes.constData(),
                                          identityFileBytes.isEmpty()
                                              ? nullptr
                                              : identityFileBytes.constData(),
                                          workspaceIdBytes.constData());
          const auto json = takeFfiString(raw, freeString, &compromiseError);
          const auto compromiseValue =
              compromiseError.isEmpty()
                  ? resultValueFromJson(json, &compromiseError)
                  : QJsonObject();
          compromiseSummary = compromiseResponseSummaryText(compromiseValue);
        }
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, deviceValue, deviceError, summaries, summariesError,
           selectedWorkspaceId, snapshotValue, snapshotError, compromiseSummary,
           compromiseError, requestedWorkspaceId, generation,
           summariesGeneration]() {
            if (guard.isNull()) {
              return;
            }
            if (guard->m_workspaceId != requestedWorkspaceId ||
                generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              return;
            }

            if (!deviceValue.isEmpty()) {
              const auto deviceId =
                  deviceValue.value(QStringLiteral("deviceId")).toString();
              if (deviceId != guard->m_deviceId) {
                guard->m_deviceId = deviceId;
                emit guard->deviceIdChanged();
              }
            }

            if (guard->m_workspaceSummariesGeneration == summariesGeneration) {
              if (!summariesError.isEmpty()) {
                guard->setSyncStatus(summariesError);
              } else if (summaries != guard->m_workspaceSummaries) {
                guard->m_workspaceSummaries = summaries;
                emit guard->workspaceSummariesChanged();
              }
            }

            if (guard->m_workspaceId.isEmpty() &&
                !selectedWorkspaceId.isEmpty()) {
              guard->m_workspaceId = selectedWorkspaceId;
              guard->persistDesktopConfig();
              emit guard->selectedWorkspaceChanged();
              emit guard->runtimeWorkspaceChanged();
            }

            if (!snapshotValue.isEmpty()) {
              guard->applyRuntimeSnapshot(snapshotValue, true);
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              if (!compromiseSummary.isEmpty()) {
                guard->setSyncStatus(QStringLiteral("local event log, ") +
                                     compromiseSummary);
              } else if (!compromiseError.isEmpty()) {
                guard->setSyncStatus(QStringLiteral("security check failed: ") +
                                     compromiseError);
              }
              return;
            }

            if (!snapshotError.isEmpty()) {
              guard->setSyncStatus(snapshotError);
            } else if (!deviceError.isEmpty()) {
              guard->setSyncStatus(deviceError);
            } else if (guard->m_workspaceId.isEmpty()) {
              guard->setSyncStatus(QStringLiteral("create a workspace first"));
            } else {
              guard->setSyncStatus(QStringLiteral("local runtime ready"));
            }
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runPublishQueueRefresh(quint64 generation, const QString &workspaceId) {
    const QPointer<ChaftController> guard(this);
    const auto queueFn = m_workspacePublishQueueJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    auto *thread = QThread::create([guard, queueFn, freeString, runtimeDir,
                                    identityFile, workspaceId, generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      char *raw = queueFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation != guard->m_publishQueueGeneration ||
                guard->m_workspaceId != workspaceId) {
              return;
            }

            if (value.isEmpty()) {
              guard->handleRuntimeUnlockFailure(error);
              QVariantMap queue;
              queue.insert(QStringLiteral("workspaceId"), workspaceId);
              if (!error.isEmpty()) {
                queue.insert(QStringLiteral("error"), error);
              }
              guard->setPublishQueue(queue);
              return;
            }

            guard->setPublishQueue(value.toVariantMap());
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceStorageHealthRefresh(quint64 generation,
                                        const QString &workspaceId) {
    const QPointer<ChaftController> guard(this);
    const auto healthFn = m_workspaceStorageHealthJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    auto *thread = QThread::create([guard, healthFn, freeString, runtimeDir,
                                    identityFile, workspaceId, generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      char *raw = healthFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation != guard->m_workspaceStorageHealthGeneration ||
                guard->m_workspaceId != workspaceId) {
              return;
            }

            if (value.isEmpty()) {
              guard->handleRuntimeUnlockFailure(error);
              QVariantMap health;
              health.insert(QStringLiteral("workspaceId"), workspaceId);
              if (!error.isEmpty()) {
                health.insert(QStringLiteral("error"), error);
              }
              guard->setWorkspaceStorageHealth(health);
              return;
            }

            guard->setWorkspaceStorageHealth(value.toVariantMap());
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceStorageMetadataRepair() {
    setSyncInFlight(true);
    const auto generation = ++m_workspaceStorageHealthGeneration;
    const QPointer<ChaftController> guard(this);
    const auto repairFn = m_repairWorkspaceStorageMetadataJson;
    const auto healthFn = m_workspaceStorageHealthJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, repairFn, healthFn, freeString,
                                    runtimeDir, identityFile, workspaceId,
                                    generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      char *raw = repairFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject healthValue;
      QString healthError;
      if (!value.isEmpty() && healthFn != nullptr) {
        const auto healthJson =
            takeFfiString(healthFn(runtimeDirBytes.constData(),
                                   identityFileBytes.isEmpty()
                                       ? nullptr
                                       : identityFileBytes.constData(),
                                   workspaceIdBytes.constData()),
                          freeString, &healthError);
        if (healthError.isEmpty()) {
          healthValue = resultValueFromJson(healthJson, &healthError);
        }
      }

      const auto repairedCount =
          jsonCountOrArraySize(value, QStringLiteral("repairedMetadataCount"),
                               QStringLiteral("repairedMetadataEventIds"));
      const auto promotedCount = jsonCountOrArraySize(
          value, QStringLiteral("promotedServableMetadataCount"),
          QStringLiteral("promotedServableEventIds"));
      const auto clearedCount = jsonCountOrArraySize(
          value, QStringLiteral("clearedUnservableMetadataCount"),
          QStringLiteral("clearedUnservableEventIds"));
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, healthValue, error, healthError, workspaceId,
           generation, repairedCount, promotedCount, clearedCount]() {
            if (guard.isNull()) {
              return;
            }

            guard->setSyncInFlight(false);
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(QStringLiteral(
                  "cache repair completed after workspace switch"));
              return;
            }
            if (value.isEmpty()) {
              guard->handleRuntimeUnlockFailure(error);
              QVariantMap health;
              health.insert(QStringLiteral("workspaceId"), workspaceId);
              if (!error.isEmpty()) {
                health.insert(QStringLiteral("error"), error);
              }
              guard->setWorkspaceStorageHealth(health);
              guard->setSyncStatus(error);
              return;
            }

            if (generation == guard->m_workspaceStorageHealthGeneration) {
              if (!healthValue.isEmpty()) {
                guard->setWorkspaceStorageHealth(healthValue.toVariantMap());
              } else if (!healthError.isEmpty()) {
                QVariantMap health;
                health.insert(QStringLiteral("workspaceId"), workspaceId);
                health.insert(QStringLiteral("error"), healthError);
                guard->setWorkspaceStorageHealth(health);
              }
            }

            guard->setSyncStatus(
                QStringLiteral(
                    "repaired %1 cache row(s), promoted %2, cleared %3")
                    .arg(repairedCount)
                    .arg(promotedCount)
                    .arg(clearedCount));
            guard->queueRuntimeSnapshotRefreshIfCurrent(true, workspaceId);
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runStoreSnapshotHydration(quint64 generation,
                                 const QString &workspaceId) {
    const QPointer<ChaftController> guard(this);
    const auto snapshotFn = m_storeSnapshotJson;
    const auto snapshotLatestFn = m_storeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto eventStorePath = m_eventStorePath;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, snapshotFn, snapshotLatestFn,
                                    freeString, eventStorePath, workspaceId,
                                    generation, timelineLimit]() {
      const auto eventStorePathBytes = eventStorePath.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      char *raw =
          snapshotLatestFn != nullptr
              ? snapshotLatestFn(eventStorePathBytes.constData(),
                                 workspaceIdBytes.constData(), timelineLimit)
              : snapshotFn(eventStorePathBytes.constData(),
                           workspaceIdBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration ||
                guard->m_workspaceId != workspaceId) {
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            guard->applyRuntimeSnapshot(value, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("event store snapshot"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runStoreTimelinePageLoad(qulonglong timelineStart,
                                qulonglong timelineCount, quint64 generation) {
    setSyncInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto snapshotFn = m_storeSnapshotWindowJson;
    const auto freeString = m_freeString;
    const auto eventStorePath = m_eventStorePath;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, snapshotFn, freeString,
                                    eventStorePath, workspaceId, timelineStart,
                                    timelineCount, generation]() {
      const auto eventStorePathBytes = eventStorePath.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      char *raw = snapshotFn(eventStorePathBytes.constData(),
                             workspaceIdBytes.constData(),
                             static_cast<std::size_t>(timelineStart),
                             static_cast<std::size_t>(timelineCount));

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, generation]() {
            if (guard.isNull()) {
              return;
            }

            guard->setSyncInFlight(false);
            if (guard->m_timelinePageGeneration != generation) {
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(QStringLiteral(
                  "timeline page ignored after workspace switch"));
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            const auto timelineCount =
                value.value(QStringLiteral("timeline")).toArray().size();
            guard->prependTimelineWindow(value.toVariantMap());
            guard->setSyncStatus(QStringLiteral("loaded %1 older message(s)")
                                     .arg(timelineCount));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runRuntimeSnapshotRefresh(quint64 generation, bool updateStatus) {
    const QPointer<ChaftController> guard(this);
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, snapshotFn, snapshotLatestFn, freeString, runtimeDir,
         identityFile, workspaceId, generation, updateStatus, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          QString error;
          const auto value = latestRuntimeSnapshotValue(
              snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
              identityFileBytes, workspaceIdBytes, timelineLimit, &error);

          if (guard.isNull()) {
            return;
          }
          QMetaObject::invokeMethod(
              guard.data(),
              [guard, value, error, workspaceId, generation, updateStatus]() {
                if (guard.isNull()) {
                  return;
                }
                if (generation < guard->m_lastAppliedRuntimeWriteGeneration ||
                    guard->m_workspaceId != workspaceId) {
                  return;
                }
                if (value.isEmpty()) {
                  guard->setSyncStatus(error);
                  return;
                }

                guard->applyRuntimeSnapshot(value, updateStatus);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runDeviceKeyPackagePublish(const QString &protocol,
                                  const QString &filePath, quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto publishFn = m_publishDeviceKeyPackageJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, publishFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, protocol,
                                    filePath, generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto protocolBytes = protocol.toUtf8();
      const auto filePathBytes = filePath.toUtf8();
      char *raw = publishFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), protocolBytes.constData(),
          filePathBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           generation]() {
            if (guard.isNull()) {
              return;
            }
            guard->finishRuntimeWriteSnapshot(
                value, snapshotValue, error, snapshotError, workspaceId,
                generation, QStringLiteral("key package published"),
                QStringLiteral("key package published after workspace switch"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runPeerEndpointPublish(const QString &endpointId,
                              const QString &endpoint, const QString &transport,
                              bool isBackupPeer, bool hasExpiresAtMs,
                              qint64 expiresAtMs, const QString &successStatus,
                              quint64 generation,
                              const QString &replicaStorageClass = QString(),
                              const QString &replicaRetentionHint = QString()) {
    QString metadataError;
    if (!validatePeerEndpointForPublish(endpointId, endpoint, transport,
                                        &metadataError)) {
      setSyncStatus(metadataError);
      return;
    }

    const QPointer<ChaftController> guard(this);
    const auto publishFn = m_publishPeerEndpointJson;
    const auto publishWithCapabilityFn =
        m_publishPeerEndpointWithReplicaCapabilityJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, publishFn, publishWithCapabilityFn,
                                    snapshotFn, snapshotLatestFn, freeString,
                                    runtimeDir, identityFile, workspaceId,
                                    endpointId, endpoint, transport,
                                    isBackupPeer, hasExpiresAtMs, expiresAtMs,
                                    successStatus, generation, timelineLimit,
                                    replicaStorageClass,
                                    replicaRetentionHint]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto endpointIdBytes = endpointId.toUtf8();
      const auto endpointBytes = endpoint.toUtf8();
      const auto transportBytes = transport.toUtf8();
      const auto replicaStorageClassBytes =
          replicaStorageClass.trimmed().toUtf8();
      const auto replicaRetentionHintBytes =
          replicaRetentionHint.trimmed().toUtf8();
      char *raw = nullptr;
      if (publishWithCapabilityFn != nullptr) {
        raw = publishWithCapabilityFn(
            runtimeDirBytes.constData(),
            identityFileBytes.isEmpty() ? nullptr
                                        : identityFileBytes.constData(),
            workspaceIdBytes.constData(), endpointIdBytes.constData(),
            endpointBytes.constData(), transportBytes.constData(), isBackupPeer,
            hasExpiresAtMs, expiresAtMs,
            replicaStorageClassBytes.isEmpty()
                ? nullptr
                : replicaStorageClassBytes.constData(),
            replicaRetentionHintBytes.isEmpty()
                ? nullptr
                : replicaRetentionHintBytes.constData());
      } else {
        raw = publishFn(
            runtimeDirBytes.constData(),
            identityFileBytes.isEmpty() ? nullptr
                                        : identityFileBytes.constData(),
            workspaceIdBytes.constData(), endpointIdBytes.constData(),
            endpointBytes.constData(), transportBytes.constData(), isBackupPeer,
            hasExpiresAtMs, expiresAtMs);
      }

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           successStatus, generation]() {
            if (guard.isNull()) {
              return;
            }
            guard->finishRuntimeWriteSnapshot(
                value, snapshotValue, error, snapshotError, workspaceId,
                generation, successStatus,
                QStringLiteral("endpoint published after workspace switch"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceCreate(const QString &workspaceName,
                          const QString &channelName,
                          const QString &previousWorkspaceId,
                          bool hadRuntimeWorkspace, quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto createFn = m_createWorkspaceJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, createFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceName, channelName,
                                    previousWorkspaceId, hadRuntimeWorkspace,
                                    generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceNameBytes = workspaceName.toUtf8();
      const auto channelNameBytes = channelName.toUtf8();
      char *raw = createFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceNameBytes.constData(), channelNameBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      const auto createdWorkspaceId =
          value.value(QStringLiteral("workspaceId")).toString();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!createdWorkspaceId.isEmpty()) {
        const auto createdWorkspaceIdBytes = createdWorkspaceId.toUtf8();
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, createdWorkspaceIdBytes, timelineLimit,
            &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError,
           createdWorkspaceId, previousWorkspaceId, hadRuntimeWorkspace,
           generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              if (!createdWorkspaceId.isEmpty()) {
                guard->queueWorkspaceSummariesRefresh();
                guard->queueRuntimeSnapshotRefreshIfCurrent(true,
                                                            createdWorkspaceId);
              }
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (createdWorkspaceId.isEmpty()) {
              guard->setSyncStatus(
                  QStringLiteral("workspace creation returned no workspace"));
              return;
            }
            if (guard->m_workspaceId != previousWorkspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("workspace created after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->m_workspaceId = createdWorkspaceId;
            guard->persistDesktopConfig();
            guard->applyWorkspaceLoadingSnapshot(createdWorkspaceId);
            emit guard->selectedWorkspaceChanged();
            if (!hadRuntimeWorkspace) {
              emit guard->runtimeWorkspaceChanged();
            }

            if (!snapshotValue.isEmpty()) {
              guard->applyRuntimeSnapshot(snapshotValue, false);
            } else {
              guard->queueWorkspaceSummariesRefresh();
              if (!snapshotError.isEmpty()) {
                guard->setSyncStatus(snapshotError);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                return;
              }
            }
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("workspace created"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void applyWorkspaceSummariesResult(const QVariantList &summaries,
                                     const QString &error) {
    if (!error.isEmpty()) {
      setSyncStatus(error);
      return;
    }
    if (summaries != m_workspaceSummaries) {
      m_workspaceSummaries = summaries;
      emit workspaceSummariesChanged();
    }
  }

  static QJsonObject resultValueFromWorkerJson(const QByteArray &json,
                                               QString *error) {
    if (json.isEmpty()) {
      if (error != nullptr && error->isEmpty()) {
        *error = QStringLiteral("empty FFI result");
      }
      return {};
    }
    return resultValueFromJson(json, error);
  }

  static QByteArray takeWorkerFfiString(char *raw, FreeStringFn freeString,
                                        QString *error = nullptr) {
    return takeBoundedFfiString(raw, freeString, kMaxDesktopFfiJsonBytes,
                                QStringLiteral("desktop worker FFI result"),
                                error);
  }

  static void stopWorkerPeer(RuntimeStopDirectPeerResultJsonFn stopFn,
                             FreeStringFn freeString, const QString &peerId) {
    if (stopFn == nullptr || freeString == nullptr || peerId.isEmpty()) {
      return;
    }

    const auto peerIdBytes = peerId.toUtf8();
    const auto json =
        takeWorkerFfiString(stopFn(peerIdBytes.constData()), freeString);
    Q_UNUSED(json);
  }

  void expireHostedPeerEndpointBlocking() {
    if (!hasRuntimeWorkspace() || m_publishPeerEndpointJson == nullptr ||
        m_freeString == nullptr || m_hostedPeerEndpointId.isEmpty() ||
        m_hostedPeerEndpoint.trimmed().isEmpty() ||
        m_hostedPeerTransport.isEmpty()) {
      return;
    }
    QString metadataError;
    const auto normalizedEndpoint = m_hostedPeerEndpoint.trimmed();
    if (!validatePeerEndpointForPublish(
            m_hostedPeerEndpointId, normalizedEndpoint, m_hostedPeerTransport,
            &metadataError)) {
      setSyncStatus(metadataError);
      return;
    }

    const auto runtimeDirBytes = m_runtimeDir.toUtf8();
    const auto identityFileBytes = m_identityFile.toUtf8();
    const auto workspaceIdBytes = m_workspaceId.toUtf8();
    const auto endpointIdBytes = m_hostedPeerEndpointId.toUtf8();
    const auto endpointBytes = normalizedEndpoint.toUtf8();
    const auto transportBytes = m_hostedPeerTransport.toUtf8();
    const auto replicaStorageClassBytes = QByteArray("ephemeral_peer");
    const auto replicaRetentionHintBytes = QByteArray("session");
    char *raw = nullptr;
    if (m_publishPeerEndpointWithReplicaCapabilityJson != nullptr) {
      raw = m_publishPeerEndpointWithReplicaCapabilityJson(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), endpointIdBytes.constData(),
          endpointBytes.constData(), transportBytes.constData(), false, true,
          QDateTime::currentMSecsSinceEpoch(),
          replicaStorageClassBytes.constData(),
          replicaRetentionHintBytes.constData());
    } else {
      raw = m_publishPeerEndpointJson(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), endpointIdBytes.constData(),
          endpointBytes.constData(), transportBytes.constData(), false, true,
          QDateTime::currentMSecsSinceEpoch());
    }
    const auto json = takeWorkerFfiString(raw, m_freeString);
    Q_UNUSED(json);
  }

  void stopLocalPeerBlocking() {
    if (m_hostedPeerId.isEmpty()) {
      return;
    }

    expireHostedPeerEndpointBlocking();

    if (m_stopDirectPeerJson == nullptr || m_freeString == nullptr) {
      return;
    }

    const auto peerIdBytes = m_hostedPeerId.toUtf8();
    const auto json = takeWorkerFfiString(
        m_stopDirectPeerJson(peerIdBytes.constData()), m_freeString);
    Q_UNUSED(json);
    m_hostedPeerId.clear();
    m_hostedPeerEndpoint.clear();
    m_hostedPeerEndpointId.clear();
    m_hostedPeerTransport.clear();
  }

  void publishHostedPeerEndpoint(const QString &endpointId,
                                 const QString &endpoint,
                                 const QString &transport,
                                 const QString &successStatus) {
    if (!hasRuntimeWorkspace() || m_publishPeerEndpointJson == nullptr ||
        endpoint.trimmed().isEmpty()) {
      return;
    }
    QString metadataError;
    const auto normalizedEndpoint = endpoint.trimmed();
    if (!validatePeerEndpointForPublish(endpointId, normalizedEndpoint,
                                        transport, &metadataError)) {
      setSyncStatus(metadataError);
      return;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    runPeerEndpointPublish(
        endpointId, normalizedEndpoint, transport, false, true,
        hostedPeerEndpointExpiresAtMs(),
        successStatus.isEmpty()
            ? QStringLiteral("serving %1, endpoint announced")
                  .arg(normalizedEndpoint)
            : successStatus,
        generation, QStringLiteral("ephemeral_peer"), QStringLiteral("session"));
  }

  void expireHostedPeerEndpoint(const QString &endpointId,
                                const QString &endpoint,
                                const QString &transport) {
    if (!hasRuntimeWorkspace() || m_publishPeerEndpointJson == nullptr ||
        endpointId.isEmpty() || endpoint.trimmed().isEmpty() ||
        transport.isEmpty()) {
      return;
    }
    QString metadataError;
    const auto normalizedEndpoint = endpoint.trimmed();
    if (!validatePeerEndpointForPublish(endpointId, normalizedEndpoint,
                                        transport, &metadataError)) {
      setSyncStatus(metadataError);
      return;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    runPeerEndpointPublish(endpointId, normalizedEndpoint, transport, false,
                           true, QDateTime::currentMSecsSinceEpoch(),
                           QStringLiteral("hosted endpoint expired"),
                           generation, QStringLiteral("ephemeral_peer"),
                           QStringLiteral("session"));
  }

  void runDirectPeerStart(const QString &listenEndpoint) {
    setPeerHostingInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto startFn = m_startDirectPeerJson;
    const auto stopFn = m_stopDirectPeerJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    auto *thread = QThread::create([guard, startFn, stopFn, freeString,
                                    runtimeDir, identityFile,
                                    listenEndpoint]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto listenBytes = listenEndpoint.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          startFn(runtimeDirBytes.constData(),
                  identityFileBytes.isEmpty() ? nullptr
                                              : identityFileBytes.constData(),
                  listenBytes.constData()),
          freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      const auto peerId = value.value(QStringLiteral("peerId")).toString();
      const auto endpoint = value.value(QStringLiteral("endpoint")).toString();
      if (!value.isEmpty() && (peerId.isEmpty() || endpoint.isEmpty())) {
        stopWorkerPeer(stopFn, freeString, peerId);
      }
      if (guard.isNull()) {
        stopWorkerPeer(stopFn, freeString, peerId);
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, peerId, endpoint]() {
            if (guard.isNull()) {
              return;
            }
            guard->setPeerHostingInFlight(false);
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (peerId.isEmpty() || endpoint.isEmpty()) {
              guard->setSyncStatus(
                  QStringLiteral("peer start returned no endpoint"));
              return;
            }

            guard->m_hostedPeerId = peerId;
            guard->m_hostedPeerEndpoint = endpoint;
            guard->m_hostedPeerEndpointId = QStringLiteral("hosted-direct");
            guard->m_hostedPeerTransport = QStringLiteral("direct-tcp");
            emit guard->hostedPeerChanged();
            guard->setSyncStatus(QStringLiteral("serving %1").arg(endpoint));
            guard->publishHostedPeerEndpoint(
                guard->m_hostedPeerEndpointId, endpoint,
                guard->m_hostedPeerTransport,
                QStringLiteral("serving %1, endpoint announced").arg(endpoint));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runIrohPeerStart() {
    setPeerHostingInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto startFn = m_startIrohPeerJson;
    const auto stopFn = m_stopDirectPeerJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    auto *thread = QThread::create([guard, startFn, stopFn, freeString,
                                    runtimeDir, identityFile]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          startFn(runtimeDirBytes.constData(),
                  identityFileBytes.isEmpty() ? nullptr
                                              : identityFileBytes.constData()),
          freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      const auto peerId = value.value(QStringLiteral("peerId")).toString();
      const auto endpoint = value.value(QStringLiteral("endpoint")).toString();
      if (!value.isEmpty() && (peerId.isEmpty() || endpoint.isEmpty())) {
        stopWorkerPeer(stopFn, freeString, peerId);
      }
      if (guard.isNull()) {
        stopWorkerPeer(stopFn, freeString, peerId);
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, peerId, endpoint]() {
            if (guard.isNull()) {
              return;
            }
            guard->setPeerHostingInFlight(false);
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (peerId.isEmpty() || endpoint.isEmpty()) {
              guard->setSyncStatus(
                  QStringLiteral("Iroh peer start returned no endpoint"));
              return;
            }

            guard->m_hostedPeerId = peerId;
            guard->m_hostedPeerEndpoint = endpoint;
            guard->m_hostedPeerEndpointId = QStringLiteral("hosted-iroh");
            guard->m_hostedPeerTransport = QStringLiteral("iroh");
            emit guard->hostedPeerChanged();
            guard->setSyncStatus(QStringLiteral("serving %1").arg(endpoint));
            guard->publishHostedPeerEndpoint(
                guard->m_hostedPeerEndpointId, endpoint,
                guard->m_hostedPeerTransport,
                QStringLiteral("serving %1, endpoint announced").arg(endpoint));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runPeerStop(const QString &peerId, const QString &endpoint,
                   const QString &endpointId, const QString &transport) {
    setPeerHostingInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto stopFn = m_stopDirectPeerJson;
    const auto freeString = m_freeString;
    auto *thread = QThread::create([guard, stopFn, freeString, peerId, endpoint,
                                    endpointId, transport]() {
      const auto peerIdBytes = peerId.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(stopFn(peerIdBytes.constData()),
                                            freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, peerId, endpoint, endpointId, transport]() {
            if (guard.isNull()) {
              return;
            }
            guard->setPeerHostingInFlight(false);
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            if (guard->m_hostedPeerId == peerId) {
              guard->expireHostedPeerEndpoint(endpointId, endpoint, transport);
              guard->m_hostedPeerId.clear();
              guard->m_hostedPeerEndpoint.clear();
              guard->m_hostedPeerEndpointId.clear();
              guard->m_hostedPeerTransport.clear();
              emit guard->hostedPeerChanged();
            }
            guard->setSyncStatus(
                endpoint.isEmpty()
                    ? QStringLiteral("peer stopped")
                    : QStringLiteral("stopped %1").arg(endpoint));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceJsonExport(RuntimeExportWorkspaceKeyResultJsonFn exportFn,
                              const QString &successStatus) {
    setKeyTransferJson(QString());
    setKeyTransferInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, exportFn, freeString, runtimeDir,
                                    identityFile, workspaceId,
                                    successStatus]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          exportFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData()),
          freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      const auto exportedJson =
          value.isEmpty() ? QString()
                          : QString::fromUtf8(QJsonDocument(value).toJson(
                                QJsonDocument::Compact));

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, exportedJson, error, successStatus]() {
            if (guard.isNull()) {
              return;
            }
            guard->setKeyTransferInFlight(false);
            if (exportedJson.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            guard->setKeyTransferJson(exportedJson);
            guard->setSyncStatus(successStatus);
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelKeyExport(const QString &channelId,
                           const QString &successStatus) {
    setKeyTransferJson(QString());
    setKeyTransferInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto exportFn = m_exportChannelKeyJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, exportFn, freeString, runtimeDir,
                                    identityFile, workspaceId, channelId,
                                    successStatus]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto channelIdBytes = channelId.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          exportFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData(), channelIdBytes.constData()),
          freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      const auto exportedJson =
          value.isEmpty() ? QString()
                          : QString::fromUtf8(QJsonDocument(value).toJson(
                                QJsonDocument::Compact));

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, exportedJson, error, successStatus]() {
            if (guard.isNull()) {
              return;
            }
            guard->setKeyTransferInFlight(false);
            if (exportedJson.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            guard->setKeyTransferJson(exportedJson);
            guard->setSyncStatus(successStatus);
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelKeyRotation(const QString &channelId, quint64 generation) {
    setKeyTransferJson(QString());
    setKeyTransferInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto rotateFn = m_rotateChannelKeyJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, rotateFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, channelId,
                                    generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto channelIdBytes = channelId.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          rotateFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData(), channelIdBytes.constData()),
          freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      const auto rotationJson =
          value.isEmpty() ? QString()
                          : QString::fromUtf8(QJsonDocument(value).toJson(
                                QJsonDocument::Compact));
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, rotationJson, snapshotValue, error, snapshotError,
           workspaceId, generation]() {
            if (guard.isNull()) {
              return;
            }
            guard->setKeyTransferInFlight(false);
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("channel key rotated after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }
            guard->setKeyTransferJson(rotationJson);
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("channel key rotated"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runRecoveryBundleExport(const QString &passphrase,
                               const QString &successStatus) {
    setKeyTransferJson(QString());
    setKeyTransferInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto exportFn = m_exportRecoveryBundleJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, exportFn, freeString, runtimeDir,
                                    identityFile, workspaceId, passphrase,
                                    successStatus]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto passphraseBytes = passphrase.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          exportFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData(), passphraseBytes.constData()),
          freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      const auto exportedJson =
          value.isEmpty() ? QString()
                          : QString::fromUtf8(QJsonDocument(value).toJson(
                                QJsonDocument::Compact));

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, exportedJson, error, successStatus]() {
            if (guard.isNull()) {
              return;
            }
            guard->setKeyTransferInFlight(false);
            if (exportedJson.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            guard->setKeyTransferJson(exportedJson);
            guard->setSyncStatus(successStatus);
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceCompromiseRotation(quint64 generation) {
    setKeyTransferJson(QString());
    setKeyTransferInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto rotateFn = m_rotateWorkspaceForSuspectedCompromiseJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, rotateFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, generation,
                                    timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          rotateFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData()),
          freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      const auto rotationJson =
          value.isEmpty() ? QString()
                          : QString::fromUtf8(QJsonDocument(value).toJson(
                                QJsonDocument::Compact));
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, rotationJson, snapshotValue, error, snapshotError,
           workspaceId, generation]() {
            if (guard.isNull()) {
              return;
            }
            guard->setKeyTransferInFlight(false);
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("keys rotated after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }
            guard->setKeyTransferJson(rotationJson);
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("keys rotated"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceCompromiseDetection() {
    setKeyTransferInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto detectFn = m_detectCompromiseJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, detectFn, freeString, runtimeDir,
                                    identityFile, workspaceId]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          detectFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData()),
          freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      const auto status =
          value.isEmpty() ? error : compromiseReportSummaryText(value);

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, status, workspaceId]() {
            if (guard.isNull()) {
              return;
            }
            guard->setKeyTransferInFlight(false);
            if (status.isEmpty()) {
              guard->setSyncStatus(QStringLiteral("security review failed"));
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(QStringLiteral(
                  "security review completed after workspace switch"));
              return;
            }
            guard->setSyncStatus(status);
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceKeyImport(RuntimeImportWorkspaceKeyResultJsonFn importFn,
                             const QString &keyJson,
                             const QString &previousWorkspaceId,
                             bool hadRuntimeWorkspace, quint64 generation,
                             const QString &successStatus) {
    setKeyTransferInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto listFn = m_listWorkspacesJson;
    const auto listPageFn = m_listWorkspacePageJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, importFn, listFn, listPageFn,
                                    snapshotFn, snapshotLatestFn, freeString,
                                    runtimeDir, identityFile, keyJson,
                                    previousWorkspaceId, hadRuntimeWorkspace,
                                    generation, successStatus,
                                    timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto keyBytes = keyJson.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          importFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   keyBytes.constData()),
          freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      const auto importedWorkspaceId =
          value.value(QStringLiteral("workspaceId")).toString();
      QVariantList summaries;
      QString summariesError;
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        summaries = workspaceSummariesFromRuntime(
            listFn, listPageFn, freeString, runtimeDirBytes, identityFileBytes,
            &summariesError);
        if (!importedWorkspaceId.isEmpty()) {
          const auto workspaceIdBytes = importedWorkspaceId.toUtf8();
          snapshotValue = latestRuntimeSnapshotValue(
              snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
              identityFileBytes, workspaceIdBytes, timelineLimit,
              &snapshotError);
        }
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, importedWorkspaceId, summaries, summariesError,
           snapshotValue, snapshotError, previousWorkspaceId,
           hadRuntimeWorkspace, generation, successStatus]() {
            if (guard.isNull()) {
              return;
            }
            guard->setKeyTransferInFlight(false);
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              if (!importedWorkspaceId.isEmpty()) {
                guard->queueWorkspaceSummariesRefresh();
                guard->queueRuntimeSnapshotRefreshIfCurrent(
                    true, importedWorkspaceId);
              }
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (importedWorkspaceId.isEmpty()) {
              guard->setSyncStatus(
                  QStringLiteral("key import returned no workspace"));
              return;
            }

            guard->applyWorkspaceSummariesResult(summaries, summariesError);
            if (guard->m_workspaceId != previousWorkspaceId) {
              guard->setSyncStatus(successStatus +
                                   QStringLiteral(" after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->m_workspaceId = importedWorkspaceId;
            guard->persistDesktopConfig();
            guard->applyWorkspaceLoadingSnapshot(importedWorkspaceId);
            emit guard->selectedWorkspaceChanged();
            if (!hadRuntimeWorkspace) {
              emit guard->runtimeWorkspaceChanged();
            }

            if (!snapshotValue.isEmpty()) {
              guard->applyRuntimeSnapshot(snapshotValue, false);
            } else {
              guard->queueWorkspaceSummariesRefresh();
              Q_UNUSED(snapshotError);
            }
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(successStatus);
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelKeyImport(const QString &keyJson,
                           const QString &previousWorkspaceId,
                           bool hadRuntimeWorkspace, quint64 generation) {
    setKeyTransferInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto importFn = m_importChannelKeyJson;
    const auto listFn = m_listWorkspacesJson;
    const auto listPageFn = m_listWorkspacePageJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, importFn, listFn, listPageFn, snapshotFn, snapshotLatestFn,
         freeString, runtimeDir, identityFile, keyJson, previousWorkspaceId,
         hadRuntimeWorkspace, generation, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto keyBytes = keyJson.toUtf8();
          QString error;
          const auto json =
              takeWorkerFfiString(importFn(runtimeDirBytes.constData(),
                                           identityFileBytes.isEmpty()
                                               ? nullptr
                                               : identityFileBytes.constData(),
                                           keyBytes.constData()),
                                  freeString, &error);
          const auto value = resultValueFromWorkerJson(json, &error);
          const auto importedWorkspaceId =
              value.value(QStringLiteral("workspaceId")).toString();
          const auto snapshotWorkspaceId =
              hadRuntimeWorkspace ? previousWorkspaceId : importedWorkspaceId;
          QVariantList summaries;
          QString summariesError;
          QJsonObject snapshotValue;
          QString snapshotError;
          if (!value.isEmpty()) {
            summaries = workspaceSummariesFromRuntime(
                listFn, listPageFn, freeString, runtimeDirBytes,
                identityFileBytes, &summariesError);
            if (!snapshotWorkspaceId.isEmpty()) {
              const auto workspaceIdBytes = snapshotWorkspaceId.toUtf8();
              snapshotValue = latestRuntimeSnapshotValue(
                  snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
                  identityFileBytes, workspaceIdBytes, timelineLimit,
                  &snapshotError);
            }
          }

          if (guard.isNull()) {
            return;
          }
          QMetaObject::invokeMethod(
              guard.data(),
              [guard, value, error, importedWorkspaceId, snapshotWorkspaceId,
               summaries, summariesError, snapshotValue, snapshotError,
               previousWorkspaceId, hadRuntimeWorkspace, generation]() {
                if (guard.isNull()) {
                  return;
                }
                guard->setKeyTransferInFlight(false);
                if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
                  guard->queueWorkspaceSummariesRefresh();
                  guard->queueRuntimeSnapshotRefreshIfCurrent(
                      !snapshotWorkspaceId.isEmpty(), snapshotWorkspaceId);
                  return;
                }
                if (value.isEmpty()) {
                  guard->setSyncStatus(error);
                  return;
                }
                if (importedWorkspaceId.isEmpty()) {
                  guard->setSyncStatus(QStringLiteral(
                      "channel key import returned no workspace"));
                  return;
                }

                guard->applyWorkspaceSummariesResult(summaries, summariesError);
                if (guard->m_workspaceId != previousWorkspaceId) {
                  guard->setSyncStatus(QStringLiteral(
                      "channel key imported after workspace switch"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }

                if (!hadRuntimeWorkspace) {
                  guard->m_workspaceId = importedWorkspaceId;
                  guard->persistDesktopConfig();
                  guard->applyWorkspaceLoadingSnapshot(importedWorkspaceId);
                  emit guard->selectedWorkspaceChanged();
                  emit guard->runtimeWorkspaceChanged();
                }

                if (!snapshotValue.isEmpty() &&
                    guard->m_workspaceId == snapshotWorkspaceId) {
                  guard->applyRuntimeSnapshot(snapshotValue, false);
                } else {
                  guard->queueWorkspaceSummariesRefresh();
                  Q_UNUSED(snapshotError);
                }
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setSyncStatus(QStringLiteral("channel key imported"));
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runRecoveryBundleImport(const QString &bundleJson,
                               const QString &passphrase,
                               const QString &previousWorkspaceId,
                               bool hadRuntimeWorkspace, quint64 generation) {
    setKeyTransferInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto importFn = m_importRecoveryBundleJson;
    const auto listFn = m_listWorkspacesJson;
    const auto listPageFn = m_listWorkspacePageJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, importFn, listFn, listPageFn,
                                    snapshotFn, snapshotLatestFn, freeString,
                                    runtimeDir, identityFile, bundleJson,
                                    passphrase, previousWorkspaceId,
                                    hadRuntimeWorkspace, generation,
                                    timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto bundleBytes = bundleJson.toUtf8();
      const auto passphraseBytes = passphrase.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          importFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   bundleBytes.constData(), passphraseBytes.constData()),
          freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      const auto importedWorkspaceId =
          value.value(QStringLiteral("workspaceId")).toString();
      QVariantList summaries;
      QString summariesError;
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        summaries = workspaceSummariesFromRuntime(
            listFn, listPageFn, freeString, runtimeDirBytes, identityFileBytes,
            &summariesError);
        if (!importedWorkspaceId.isEmpty()) {
          const auto workspaceIdBytes = importedWorkspaceId.toUtf8();
          snapshotValue = latestRuntimeSnapshotValue(
              snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
              identityFileBytes, workspaceIdBytes, timelineLimit,
              &snapshotError);
        }
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, importedWorkspaceId, summaries, summariesError,
           snapshotValue, snapshotError, previousWorkspaceId,
           hadRuntimeWorkspace, generation]() {
            if (guard.isNull()) {
              return;
            }
            guard->setKeyTransferInFlight(false);
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              if (!importedWorkspaceId.isEmpty()) {
                guard->queueWorkspaceSummariesRefresh();
                guard->queueRuntimeSnapshotRefreshIfCurrent(
                    true, importedWorkspaceId);
              }
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (importedWorkspaceId.isEmpty()) {
              guard->setSyncStatus(
                  QStringLiteral("recovery import returned no workspace"));
              return;
            }

            guard->applyWorkspaceSummariesResult(summaries, summariesError);
            if (guard->m_workspaceId != previousWorkspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("recovery bundle imported after workspace "
                                 "switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->m_workspaceId = importedWorkspaceId;
            guard->persistDesktopConfig();
            guard->applyWorkspaceLoadingSnapshot(importedWorkspaceId);
            emit guard->selectedWorkspaceChanged();
            if (!hadRuntimeWorkspace) {
              emit guard->runtimeWorkspaceChanged();
            }

            if (!snapshotValue.isEmpty()) {
              guard->applyRuntimeSnapshot(snapshotValue, false);
            } else {
              guard->queueWorkspaceSummariesRefresh();
              Q_UNUSED(snapshotError);
            }
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("recovery bundle imported"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceSearchReindex() {
    setKeyTransferInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto reindexFn = m_reindexWorkspaceSearchJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, reindexFn, freeString, runtimeDir,
                                    identityFile, workspaceId]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          reindexFn(runtimeDirBytes.constData(),
                    identityFileBytes.isEmpty() ? nullptr
                                                : identityFileBytes.constData(),
                    workspaceIdBytes.constData()),
          freeString, &error);
      const auto value = resultValueFromWorkerJson(json, &error);
      const auto status =
          value.isEmpty() ? error : reindexSearchSummaryText(value);

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, status, value, workspaceId]() {
            if (guard.isNull()) {
              return;
            }
            guard->setKeyTransferInFlight(false);
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(QStringLiteral(
                  "search reindex completed after workspace switch"));
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(status.isEmpty()
                                       ? QStringLiteral("search reindex failed")
                                       : status);
              return;
            }

            guard->setSyncStatus(status);
            guard->refreshActiveSearch();
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void
  runWorkspaceOpenMlsAction(RuntimeOpenMlsWorkspaceActionResultJsonFn actionFn,
                            const QString &successStatus, quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, actionFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, successStatus,
                                    generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          actionFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData()),
          freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           successStatus, generation]() {
            if (guard.isNull()) {
              return;
            }
            guard->finishRuntimeWriteSnapshot(
                value, snapshotValue, error, snapshotError, workspaceId,
                generation, successStatus,
                QStringLiteral(
                    "OpenMLS action completed after workspace switch"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceOpenMlsValueAction(
      RuntimeOpenMlsWorkspaceValueResultJsonFn actionFn, const QString &value,
      bool allowEmptyValue, const QString &successStatus, quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, actionFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, value,
                                    allowEmptyValue, successStatus, generation,
                                    timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto valueBytes = value.toUtf8();
      const auto valuePtr = allowEmptyValue && valueBytes.isEmpty()
                                ? nullptr
                                : valueBytes.constData();
      QString error;
      const auto json = takeWorkerFfiString(
          actionFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData(), valuePtr),
          freeString, &error);
      const auto valueObject =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!valueObject.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, valueObject, snapshotValue, error, snapshotError, workspaceId,
           successStatus, generation]() {
            if (guard.isNull()) {
              return;
            }
            guard->finishRuntimeWriteSnapshot(
                valueObject, snapshotValue, error, snapshotError, workspaceId,
                generation, successStatus,
                QStringLiteral(
                    "OpenMLS action completed after workspace switch"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelOpenMlsAction(RuntimeOpenMlsChannelActionResultJsonFn actionFn,
                               const QString &channelId,
                               const QString &successStatus,
                               quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, actionFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, channelId,
                                    successStatus, generation,
                                    timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto channelIdBytes = channelId.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          actionFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData(), channelIdBytes.constData()),
          freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           successStatus, generation]() {
            if (guard.isNull()) {
              return;
            }
            guard->finishRuntimeWriteSnapshot(
                value, snapshotValue, error, snapshotError, workspaceId,
                generation, successStatus,
                QStringLiteral(
                    "OpenMLS action completed after workspace switch"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelOpenMlsValueAction(
      RuntimeOpenMlsChannelValueResultJsonFn actionFn, const QString &channelId,
      const QString &value, bool allowEmptyValue, const QString &successStatus,
      quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, actionFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, channelId, value,
                                    allowEmptyValue, successStatus, generation,
                                    timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto channelIdBytes = channelId.toUtf8();
      const auto valueBytes = value.toUtf8();
      const auto valuePtr = allowEmptyValue && valueBytes.isEmpty()
                                ? nullptr
                                : valueBytes.constData();
      QString error;
      const auto json = takeWorkerFfiString(
          actionFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData(), channelIdBytes.constData(),
                   valuePtr),
          freeString, &error);
      const auto valueObject =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!valueObject.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, valueObject, snapshotValue, error, snapshotError, workspaceId,
           successStatus, generation]() {
            if (guard.isNull()) {
              return;
            }
            guard->finishRuntimeWriteSnapshot(
                valueObject, snapshotValue, error, snapshotError, workspaceId,
                generation, successStatus,
                QStringLiteral(
                    "OpenMLS action completed after workspace switch"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelReadMark(const QString &channelId, quint64 readGeneration,
                          quint64 writeGeneration) {
    const QPointer<ChaftController> guard(this);
    const auto markFn = m_markChannelReadJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, markFn, snapshotFn, snapshotLatestFn,
                                    freeString, runtimeDir, identityFile,
                                    workspaceId, channelId, readGeneration,
                                    writeGeneration, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto channelIdBytes = channelId.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          markFn(runtimeDirBytes.constData(),
                 identityFileBytes.isEmpty() ? nullptr
                                             : identityFileBytes.constData(),
                 workspaceIdBytes.constData(), channelIdBytes.constData()),
          freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty() &&
          !value.value(QStringLiteral("alreadyRead")).toBool()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           readGeneration, writeGeneration]() {
            if (guard.isNull()) {
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              return;
            }
            if (guard->m_readMarkerGeneration != readGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(
                  !value.isEmpty() &&
                      !value.value(QStringLiteral("alreadyRead")).toBool(),
                  workspaceId);
              return;
            }
            if (writeGeneration < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(
                  !value.isEmpty() &&
                      !value.value(QStringLiteral("alreadyRead")).toBool(),
                  workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (value.value(QStringLiteral("alreadyRead")).toBool()) {
              return;
            }
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = writeGeneration;
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runDirectEventPublishWithTrustSnapshot(const QString &eventId,
                                              const QString &peerEndpoint) {
    setSyncInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto freeString = m_freeString;
    const auto publishFn = m_publishEventWithTrustSnapshotJson;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, publishFn, freeString, runtimeDir,
                                    identityFile, workspaceId, eventId,
                                    peerEndpoint]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto eventIdBytes = eventId.toUtf8();
      const auto endpointBytes = peerEndpoint.toUtf8();
      char *raw = publishFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), eventIdBytes.constData(),
          endpointBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      const auto publishedCount =
          jsonCountOrArraySize(value, QStringLiteral("publishedEventCount"),
                               QStringLiteral("publishedEventIds"));
      const auto publishedBlobCount =
          jsonCountOrArraySize(value, QStringLiteral("publishedBlobCount"),
                               QStringLiteral("publishedBlobHashes"));
      const auto missingBlobCount =
          jsonCountOrArraySize(value, QStringLiteral("missingBlobCount"),
                               QStringLiteral("missingBlobHashes"));
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, publishedCount, publishedBlobCount, missingBlobCount,
           error]() {
            if (guard.isNull()) {
              return;
            }

            guard->setSyncInFlight(false);
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            guard->setSyncStatus(
                QStringLiteral("published proof %1 event(s), %2 blob(s), %3 "
                               "missing blob(s)")
                    .arg(publishedCount)
                    .arg(publishedBlobCount)
                    .arg(missingBlobCount));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelCreate(const QString &channelName, bool isPrivate,
                        quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto createFn = m_createChannelJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, createFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, channelName,
                                    isPrivate, generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto channelNameBytes = channelName.toUtf8();
      char *raw = createFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), channelNameBytes.constData(),
          isPrivate);

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("channel created after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("channel created"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runDeviceProfileUpdate(const QString &displayName, quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto updateFn = m_updateDeviceProfileJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, updateFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, displayName,
                                    generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto displayNameBytes = displayName.toUtf8();
      char *raw = updateFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), displayNameBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("profile updated after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("profile updated"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runMemberInvite(const QString &deviceId, const QString &role,
                       quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto inviteFn = m_inviteMemberJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, inviteFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, deviceId, role,
                                    generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto deviceIdBytes = deviceId.toUtf8();
      const auto roleBytes = role.toUtf8();
      char *raw = inviteFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), deviceIdBytes.constData(),
          roleBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("device invited after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("device invited"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runMemberRemove(const QString &deviceId, quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto openMlsFn = m_removeMemberWithOpenMlsJson;
    const auto rotationFn = m_removeMemberWithKeyRotationJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, openMlsFn, rotationFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, deviceId,
                                    generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto deviceIdBytes = deviceId.toUtf8();

      QJsonObject value;
      QString error;
      QString openMlsError;
      QString successStatus;
      if (openMlsFn != nullptr) {
        char *raw = openMlsFn(
            runtimeDirBytes.constData(),
            identityFileBytes.isEmpty() ? nullptr
                                        : identityFileBytes.constData(),
            workspaceIdBytes.constData(), deviceIdBytes.constData());
        const auto json = takeFfiString(raw, freeString, &openMlsError);
        if (openMlsError.isEmpty()) {
          value = resultValueFromJson(json, &openMlsError);
        }
        if (!value.isEmpty()) {
          successStatus =
              QStringLiteral("workspace member removed with OpenMLS");
        } else if (!shouldFallbackFromOpenMlsRemovalError(openMlsError)) {
          error = openMlsError;
        }
      }

      if (value.isEmpty() && error.isEmpty()) {
        if (rotationFn == nullptr) {
          error = openMlsError.isEmpty()
                      ? QStringLiteral("member removal unavailable")
                      : openMlsError;
        } else {
          QString rotationError;
          char *raw = rotationFn(
              runtimeDirBytes.constData(),
              identityFileBytes.isEmpty() ? nullptr
                                          : identityFileBytes.constData(),
              workspaceIdBytes.constData(), deviceIdBytes.constData());
          const auto json = takeFfiString(raw, freeString, &rotationError);
          if (rotationError.isEmpty()) {
            value = resultValueFromJson(json, &rotationError);
          }
          if (value.isEmpty()) {
            error = rotationError.isEmpty() ? openMlsError : rotationError;
          } else {
            successStatus =
                QStringLiteral("workspace member removed and keys rotated");
          }
        }
      }

      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           successStatus, generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("workspace member removed after workspace "
                                 "switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(successStatus);
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelMemberAdd(const QString &channelId, const QString &deviceId,
                           quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto addFn = m_addChannelMemberJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, addFn, snapshotFn, snapshotLatestFn,
                                    freeString, runtimeDir, identityFile,
                                    workspaceId, channelId, deviceId,
                                    generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto channelIdBytes = channelId.toUtf8();
      const auto deviceIdBytes = deviceId.toUtf8();
      char *raw = addFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), channelIdBytes.constData(),
          deviceIdBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(QStringLiteral(
                  "channel access granted after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("channel access granted"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelMemberRemove(const QString &channelId, const QString &deviceId,
                              quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto openMlsFn = m_removeChannelMemberWithOpenMlsJson;
    const auto rotationFn = m_removeChannelMemberWithKeyRotationJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, openMlsFn, rotationFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, channelId,
                                    deviceId, generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto channelIdBytes = channelId.toUtf8();
      const auto deviceIdBytes = deviceId.toUtf8();

      QJsonObject value;
      QString error;
      QString openMlsError;
      QString successStatus;
      if (openMlsFn != nullptr) {
        char *raw = openMlsFn(
            runtimeDirBytes.constData(),
            identityFileBytes.isEmpty() ? nullptr
                                        : identityFileBytes.constData(),
            workspaceIdBytes.constData(), channelIdBytes.constData(),
            deviceIdBytes.constData());
        const auto json = takeFfiString(raw, freeString, &openMlsError);
        if (openMlsError.isEmpty()) {
          value = resultValueFromJson(json, &openMlsError);
        }
        if (!value.isEmpty()) {
          successStatus = QStringLiteral("channel member removed with OpenMLS");
        } else if (!shouldFallbackFromOpenMlsRemovalError(openMlsError)) {
          error = openMlsError;
        }
      }

      if (value.isEmpty() && error.isEmpty()) {
        if (rotationFn == nullptr) {
          error = openMlsError.isEmpty()
                      ? QStringLiteral("channel removal unavailable")
                      : openMlsError;
        } else {
          QString rotationError;
          char *raw = rotationFn(
              runtimeDirBytes.constData(),
              identityFileBytes.isEmpty() ? nullptr
                                          : identityFileBytes.constData(),
              workspaceIdBytes.constData(), channelIdBytes.constData(),
              deviceIdBytes.constData());
          const auto json = takeFfiString(raw, freeString, &rotationError);
          if (rotationError.isEmpty()) {
            value = resultValueFromJson(json, &rotationError);
          }
          if (value.isEmpty()) {
            error = rotationError.isEmpty() ? openMlsError : rotationError;
          } else {
            successStatus =
                QStringLiteral("channel member removed and key rotated");
          }
        }
      }

      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           successStatus, generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("channel member removed after workspace "
                                 "switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(successStatus);
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runMessageSend(const QString &channelId, const QString &replyToMessageId,
                      const QString &text, quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto sendFn = m_sendMessageJson;
    const auto sendReplyFn = m_sendMessageReplyJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, sendFn, sendReplyFn, snapshotFn, snapshotLatestFn, freeString,
         runtimeDir, identityFile, workspaceId, channelId, replyToMessageId,
         text, generation, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto channelIdBytes = channelId.toUtf8();
          const auto replyToMessageIdBytes = replyToMessageId.toUtf8();
          const auto textBytes = text.toUtf8();
          char *raw = nullptr;
          if (!replyToMessageIdBytes.isEmpty() && sendReplyFn != nullptr) {
            raw = sendReplyFn(
                runtimeDirBytes.constData(),
                identityFileBytes.isEmpty() ? nullptr
                                            : identityFileBytes.constData(),
                workspaceIdBytes.constData(), channelIdBytes.constData(),
                replyToMessageIdBytes.constData(), textBytes.constData());
          } else {
            raw = sendFn(runtimeDirBytes.constData(),
                         identityFileBytes.isEmpty()
                             ? nullptr
                             : identityFileBytes.constData(),
                         workspaceIdBytes.constData(),
                         channelIdBytes.constData(), textBytes.constData());
          }

          QString error;
          const auto json = takeFfiString(raw, freeString, &error);
          const auto value = error.isEmpty() ? resultValueFromJson(json, &error)
                                             : QJsonObject();
          QJsonObject snapshotValue;
          QString snapshotError;
          if (!value.isEmpty()) {
            snapshotValue = latestRuntimeSnapshotValue(
                snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
                identityFileBytes, workspaceIdBytes, timelineLimit,
                &snapshotError);
          }

          if (guard.isNull()) {
            return;
          }
          QMetaObject::invokeMethod(
              guard.data(),
              [guard, value, snapshotValue, error, snapshotError, workspaceId,
               generation]() {
                if (guard.isNull()) {
                  return;
                }
                if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
                  guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                              workspaceId);
                  return;
                }
                if (value.isEmpty()) {
                  guard->setSyncStatus(error);
                  return;
                }
                if (snapshotValue.isEmpty()) {
                  guard->setSyncStatus(snapshotError);
                  return;
                }
                if (guard->m_workspaceId != workspaceId) {
                  guard->setSyncStatus(
                      QStringLiteral("message sent after workspace switch"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setSyncStatus(QStringLiteral("message sent"));
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runMessageEdit(const QString &messageId, const QString &text,
                      quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto editFn = m_editMessageJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, editFn, snapshotFn, snapshotLatestFn,
                                    freeString, runtimeDir, identityFile,
                                    workspaceId, messageId, text, generation,
                                    timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto messageIdBytes = messageId.toUtf8();
      const auto textBytes = text.toUtf8();
      char *raw = editFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), messageIdBytes.constData(),
          textBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("message edited after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("message edited"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runMessageDelete(const QString &messageId, quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto deleteFn = m_deleteMessageJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, deleteFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, messageId,
                                    generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto messageIdBytes = messageId.toUtf8();
      char *raw = deleteFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), messageIdBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("message deleted after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("message deleted"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runReactionAdd(const QString &messageId, const QString &reaction,
                      quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto reactionFn = m_addReactionJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, reactionFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, messageId,
                                    reaction, generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto messageIdBytes = messageId.toUtf8();
      const auto reactionBytes = reaction.toUtf8();
      char *raw = reactionFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), messageIdBytes.constData(),
          reactionBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("reaction added after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("reaction added"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runReactionRemove(const QString &messageId, const QString &reaction,
                         quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto reactionFn = m_removeReactionJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, reactionFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceId, messageId,
                                    reaction, generation, timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto messageIdBytes = messageId.toUtf8();
      const auto reactionBytes = reaction.toUtf8();
      char *raw = reactionFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), messageIdBytes.constData(),
          reactionBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      QJsonObject snapshotValue;
      QString snapshotError;
      if (!value.isEmpty()) {
        snapshotValue = latestRuntimeSnapshotValue(
            snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
            identityFileBytes, workspaceIdBytes, timelineLimit, &snapshotError);
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, snapshotValue, error, snapshotError, workspaceId,
           generation]() {
            if (guard.isNull()) {
              return;
            }
            if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
              guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                          workspaceId);
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (snapshotValue.isEmpty()) {
              guard->setSyncStatus(snapshotError);
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(
                  QStringLiteral("reaction removed after workspace switch"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("reaction removed"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runAttachmentSend(const QString &channelId,
                         const QString &replyToMessageId, const QString &text,
                         const QString &filePath, const QString &mediaType,
                         quint64 generation) {
    setSyncInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto sendFn = m_sendAttachmentJson;
    const auto sendReplyFn = m_sendAttachmentReplyJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, sendFn, sendReplyFn, snapshotFn, snapshotLatestFn, freeString,
         runtimeDir, identityFile, workspaceId, channelId, replyToMessageId,
         text, filePath, mediaType, generation, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto channelIdBytes = channelId.toUtf8();
          const auto replyToMessageIdBytes = replyToMessageId.toUtf8();
          const auto textBytes = text.toUtf8();
          const auto filePathBytes = filePath.toUtf8();
          const auto mediaTypeBytes = mediaType.toUtf8();
          char *raw = nullptr;
          if (!replyToMessageIdBytes.isEmpty() && sendReplyFn != nullptr) {
            raw = sendReplyFn(
                runtimeDirBytes.constData(),
                identityFileBytes.isEmpty() ? nullptr
                                            : identityFileBytes.constData(),
                workspaceIdBytes.constData(), channelIdBytes.constData(),
                replyToMessageIdBytes.constData(), textBytes.constData(),
                filePathBytes.constData(), mediaTypeBytes.constData());
          } else {
            raw = sendFn(runtimeDirBytes.constData(),
                         identityFileBytes.isEmpty()
                             ? nullptr
                             : identityFileBytes.constData(),
                         workspaceIdBytes.constData(),
                         channelIdBytes.constData(), textBytes.constData(),
                         filePathBytes.constData(), mediaTypeBytes.constData());
          }

          QString error;
          const auto json = takeFfiString(raw, freeString, &error);
          const auto value = error.isEmpty() ? resultValueFromJson(json, &error)
                                             : QJsonObject();
          QJsonObject snapshotValue;
          QString snapshotError;
          if (!value.isEmpty()) {
            snapshotValue = latestRuntimeSnapshotValue(
                snapshotFn, snapshotLatestFn, freeString, runtimeDirBytes,
                identityFileBytes, workspaceIdBytes, timelineLimit,
                &snapshotError);
          }
          if (guard.isNull()) {
            return;
          }
          QMetaObject::invokeMethod(
              guard.data(),
              [guard, value, snapshotValue, error, snapshotError, workspaceId,
               generation]() {
                if (guard.isNull()) {
                  return;
                }

                guard->setSyncInFlight(false);
                if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
                  guard->queueRuntimeSnapshotRefreshIfCurrent(!value.isEmpty(),
                                                              workspaceId);
                  return;
                }
                if (value.isEmpty()) {
                  guard->setSyncStatus(error);
                  return;
                }
                if (snapshotValue.isEmpty()) {
                  guard->setSyncStatus(snapshotError);
                  return;
                }
                if (guard->m_workspaceId != workspaceId) {
                  guard->setSyncStatus(
                      QStringLiteral("attachment sent after workspace switch"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setSyncStatus(QStringLiteral("attachment sent"));
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runAttachmentSave(const QString &messageId,
                         const QString &attachmentSelector,
                         const QString &outputPath) {
    setSyncInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto saveFn = m_saveAttachmentJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, saveFn, freeString, runtimeDir,
                                    identityFile, workspaceId, messageId,
                                    attachmentSelector, outputPath]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto messageIdBytes = messageId.toUtf8();
      const auto attachmentSelectorBytes = attachmentSelector.toUtf8();
      const auto outputPathBytes = outputPath.toUtf8();
      char *raw = saveFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), messageIdBytes.constData(),
          attachmentSelectorBytes.constData(), outputPathBytes.constData());

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error]() {
            if (guard.isNull()) {
              return;
            }

            guard->setSyncInFlight(false);
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            guard->setSyncStatus(QStringLiteral("attachment saved"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runTimelinePageLoad(qulonglong timelineStart, qulonglong timelineCount,
                           quint64 generation) {
    setSyncInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto snapshotFn = m_runtimeSnapshotWindowJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, snapshotFn, freeString, runtimeDir,
                                    identityFile, workspaceId, timelineStart,
                                    timelineCount, generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      char *raw = snapshotFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), static_cast<std::size_t>(timelineStart),
          static_cast<std::size_t>(timelineCount));

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, generation]() {
            if (guard.isNull()) {
              return;
            }

            guard->setSyncInFlight(false);
            if (guard->m_timelinePageGeneration != generation) {
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(QStringLiteral(
                  "timeline page ignored after workspace switch"));
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            const auto timelineCount =
                value.value(QStringLiteral("timeline")).toArray().size();
            guard->prependTimelineWindow(value.toVariantMap());
            guard->setSyncStatus(QStringLiteral("loaded %1 older message(s)")
                                     .arg(timelineCount));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelTimelineLatestLoad(const QString &channelId,
                                    std::size_t timelineLimit,
                                    quint64 generation) {
    setSyncInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto snapshotFn = m_runtimeChannelSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, snapshotFn, freeString, runtimeDir,
                                    identityFile, workspaceId, channelId,
                                    timelineLimit, generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto channelIdBytes = channelId.toUtf8();
      char *raw = snapshotFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), channelIdBytes.constData(),
          timelineLimit);

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, channelId, generation]() {
            if (guard.isNull()) {
              return;
            }

            guard->setSyncInFlight(false);
            if (guard->m_timelinePageGeneration != generation) {
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(QStringLiteral(
                  "channel timeline ignored after workspace switch"));
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (value.value(QStringLiteral("timelineChannelId")).toString() !=
                channelId) {
              guard->setSyncStatus(
                  QStringLiteral("channel timeline page was stale"));
              return;
            }

            guard->m_workspaceSnapshot =
                guard->snapshotWithPreservedResolvedChannels(value);
            emit guard->workspaceSnapshotChanged();
            const auto timelineCount =
                value.value(QStringLiteral("timeline")).toArray().size();
            guard->setSyncStatus(QStringLiteral("loaded %1 channel message(s)")
                                     .arg(timelineCount));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelTimelinePageLoad(const QString &channelId,
                                  qulonglong timelineStart,
                                  qulonglong timelineCount,
                                  quint64 generation) {
    setSyncInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto snapshotFn = m_runtimeChannelSnapshotWindowJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, snapshotFn, freeString, runtimeDir,
                                    identityFile, workspaceId, channelId,
                                    timelineStart, timelineCount,
                                    generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto channelIdBytes = channelId.toUtf8();
      char *raw = snapshotFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceIdBytes.constData(), channelIdBytes.constData(),
          static_cast<std::size_t>(timelineStart),
          static_cast<std::size_t>(timelineCount));

      QString error;
      const auto json = takeFfiString(raw, freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, channelId, generation]() {
            if (guard.isNull()) {
              return;
            }

            guard->setSyncInFlight(false);
            if (guard->m_timelinePageGeneration != generation) {
              return;
            }
            if (guard->m_workspaceId != workspaceId) {
              guard->setSyncStatus(QStringLiteral(
                  "channel timeline page ignored after workspace switch"));
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (value.value(QStringLiteral("timelineChannelId")).toString() !=
                channelId) {
              guard->setSyncStatus(
                  QStringLiteral("channel timeline page was stale"));
              return;
            }

            const auto timelineCount =
                value.value(QStringLiteral("timeline")).toArray().size();
            guard->prependTimelineWindow(value.toVariantMap());
            guard->setSyncStatus(
                QStringLiteral("loaded %1 older channel message(s)")
                    .arg(timelineCount));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceSearch(const QString &query, quint64 generation) {
    QString metadataError;
    if (!validateMetadataTextForWrite(
            query, kMaxSearchQueryBytes, QStringLiteral("search query"),
            QStringLiteral("512 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return;
    }

    const QPointer<ChaftController> guard(this);
    const auto searchFn = m_searchWorkspaceJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, searchFn, freeString, runtimeDir,
                                    identityFile, workspaceId, query,
                                    generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto queryBytes = query.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          searchFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData(), queryBytes.constData()),
          freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, query, generation]() {
            if (guard.isNull()) {
              return;
            }
            if (guard->m_messageSearchGeneration != generation ||
                guard->m_workspaceId != workspaceId) {
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            guard->applyWorkspaceSearchResults(query, value);
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceChannelSearch(const QString &query, quint64 generation) {
    QString metadataError;
    if (!validateMetadataTextForWrite(
            query, kMaxSearchQueryBytes, QStringLiteral("search query"),
            QStringLiteral("512 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return;
    }

    const QPointer<ChaftController> guard(this);
    const auto searchFn = m_searchWorkspaceChannelsJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    auto *thread = QThread::create([guard, searchFn, freeString, runtimeDir,
                                    identityFile, workspaceId, query,
                                    generation]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceIdBytes = workspaceId.toUtf8();
      const auto queryBytes = query.toUtf8();
      QString error;
      const auto json = takeWorkerFfiString(
          searchFn(runtimeDirBytes.constData(),
                   identityFileBytes.isEmpty() ? nullptr
                                               : identityFileBytes.constData(),
                   workspaceIdBytes.constData(), queryBytes.constData(),
                   configuredChannelPageLimit()),
          freeString, &error);
      const auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, value, error, workspaceId, query, generation]() {
            if (guard.isNull()) {
              return;
            }
            if (guard->m_channelSearchGeneration != generation ||
                guard->m_workspaceId != workspaceId) {
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }

            guard->applyWorkspaceChannelSearchResults(query, value);
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void setSyncInFlight(bool syncInFlight) {
    if (m_syncInFlight == syncInFlight) {
      return;
    }
    m_syncInFlight = syncInFlight;
    emit syncInFlightChanged();
  }

  void setChannelPageInFlight(bool channelPageInFlight) {
    m_channelPageInFlight = channelPageInFlight;
  }

  void setMemberPageInFlight(bool memberPageInFlight) {
    m_memberPageInFlight = memberPageInFlight;
  }

  void setPeerHostingInFlight(bool peerHostingInFlight) {
    if (m_peerHostingInFlight == peerHostingInFlight) {
      return;
    }
    m_peerHostingInFlight = peerHostingInFlight;
    emit peerHostingInFlightChanged();
  }

  void setKeyTransferInFlight(bool keyTransferInFlight) {
    if (m_keyTransferInFlight == keyTransferInFlight) {
      return;
    }
    m_keyTransferInFlight = keyTransferInFlight;
    emit keyTransferInFlightChanged();
  }

  void setKeyTransferJson(const QString &keyTransferJson) {
    if (m_keyTransferJson == keyTransferJson) {
      return;
    }
    m_keyTransferJson = keyTransferJson;
    emit keyTransferJsonChanged();
  }

  void setRuntimeUnlockRequired(bool runtimeUnlockRequired) {
    if (m_runtimeUnlockRequired == runtimeUnlockRequired) {
      return;
    }
    m_runtimeUnlockRequired = runtimeUnlockRequired;
    emit runtimeUnlockChanged();
  }

  void setSyncStatus(const QString &syncStatus) {
    handleRuntimeUnlockFailure(syncStatus);
    if (m_syncStatus == syncStatus) {
      return;
    }
    m_syncStatus = syncStatus;
    emit syncStatusChanged();
  }

  void setPublishQueue(const QVariantMap &publishQueue) {
    if (m_publishQueue == publishQueue) {
      return;
    }
    m_publishQueue = publishQueue;
    emit publishQueueChanged();
  }

  void setWorkspaceStorageHealth(const QVariantMap &workspaceStorageHealth) {
    if (m_workspaceStorageHealth == workspaceStorageHealth) {
      return;
    }
    m_workspaceStorageHealth = workspaceStorageHealth;
    emit workspaceStorageHealthChanged();
  }

  void clearPublishQueue() {
    ++m_publishQueueGeneration;
    setPublishQueue(QVariantMap{});
  }

  void clearWorkspaceStorageHealth() {
    ++m_workspaceStorageHealthGeneration;
    setWorkspaceStorageHealth(QVariantMap{});
  }

  QLibrary m_library;
  RuntimeSnapshotResultJsonFn m_runtimeSnapshotJson = nullptr;
  RuntimeSnapshotLatestResultJsonFn m_runtimeSnapshotLatestJson = nullptr;
  RuntimeSnapshotWindowResultJsonFn m_runtimeSnapshotWindowJson = nullptr;
  RuntimeChannelSnapshotLatestResultJsonFn m_runtimeChannelSnapshotLatestJson =
      nullptr;
  RuntimeChannelSnapshotWindowResultJsonFn m_runtimeChannelSnapshotWindowJson =
      nullptr;
  StoreSnapshotResultJsonFn m_storeSnapshotJson = nullptr;
  StoreSnapshotLatestResultJsonFn m_storeSnapshotLatestJson = nullptr;
  StoreSnapshotWindowResultJsonFn m_storeSnapshotWindowJson = nullptr;
  RuntimeDeviceIdResultJsonFn m_deviceIdJson = nullptr;
  RuntimeListWorkspacesResultJsonFn m_listWorkspacesJson = nullptr;
  RuntimeListWorkspacePageResultJsonFn m_listWorkspacePageJson = nullptr;
  RuntimeListWorkspaceChannelPageResultJsonFn m_listWorkspaceChannelPageJson =
      nullptr;
  RuntimeListWorkspaceChannelPageContainingResultJsonFn
      m_listWorkspaceChannelPageContainingJson = nullptr;
  RuntimeListWorkspaceMemberPageResultJsonFn m_listWorkspaceMemberPageJson =
      nullptr;
  RuntimeCreateWorkspaceResultJsonFn m_createWorkspaceJson = nullptr;
  RuntimeCreateChannelResultJsonFn m_createChannelJson = nullptr;
  RuntimeUpdateDeviceProfileResultJsonFn m_updateDeviceProfileJson = nullptr;
  RuntimePublishDeviceKeyPackageResultJsonFn m_publishDeviceKeyPackageJson =
      nullptr;
  RuntimePublishPeerEndpointResultJsonFn m_publishPeerEndpointJson = nullptr;
  RuntimePublishPeerEndpointWithReplicaCapabilityResultJsonFn
      m_publishPeerEndpointWithReplicaCapabilityJson = nullptr;
  RuntimeOpenMlsWorkspaceActionResultJsonFn
      m_publishOpenMlsDeviceKeyPackageJson = nullptr;
  RuntimeOpenMlsWorkspaceActionResultJsonFn m_createOpenMlsWorkspaceGroupJson =
      nullptr;
  RuntimeOpenMlsWorkspaceValueResultJsonFn
      m_addOpenMlsWorkspaceGroupMemberJson = nullptr;
  RuntimeOpenMlsWorkspaceValueResultJsonFn m_joinOpenMlsWorkspaceGroupJson =
      nullptr;
  RuntimeOpenMlsWorkspaceActionResultJsonFn m_updateOpenMlsWorkspaceGroupJson =
      nullptr;
  RuntimeOpenMlsWorkspaceActionResultJsonFn m_updateWorkspaceOpenMlsGroupsJson =
      nullptr;
  RuntimeOpenMlsWorkspaceValueResultJsonFn
      m_applyOpenMlsWorkspaceGroupCommitsJson = nullptr;
  RuntimeOpenMlsChannelActionResultJsonFn m_createOpenMlsChannelGroupJson =
      nullptr;
  RuntimeOpenMlsChannelValueResultJsonFn m_addOpenMlsChannelGroupMemberJson =
      nullptr;
  RuntimeOpenMlsChannelValueResultJsonFn m_joinOpenMlsChannelGroupJson =
      nullptr;
  RuntimeOpenMlsChannelActionResultJsonFn m_updateOpenMlsChannelGroupJson =
      nullptr;
  RuntimeOpenMlsChannelValueResultJsonFn m_applyOpenMlsChannelGroupCommitsJson =
      nullptr;
  RuntimeSendMessageResultJsonFn m_sendMessageJson = nullptr;
  RuntimeSendMessageReplyResultJsonFn m_sendMessageReplyJson = nullptr;
  RuntimeSendAttachmentResultJsonFn m_sendAttachmentJson = nullptr;
  RuntimeSendAttachmentReplyResultJsonFn m_sendAttachmentReplyJson = nullptr;
  RuntimeSaveAttachmentResultJsonFn m_saveAttachmentJson = nullptr;
  RuntimePruneBlobsResultJsonFn m_pruneBlobsJson = nullptr;
  RuntimeEditMessageResultJsonFn m_editMessageJson = nullptr;
  RuntimeDeleteMessageResultJsonFn m_deleteMessageJson = nullptr;
  RuntimeAddReactionResultJsonFn m_addReactionJson = nullptr;
  RuntimeRemoveReactionResultJsonFn m_removeReactionJson = nullptr;
  RuntimeMarkChannelReadResultJsonFn m_markChannelReadJson = nullptr;
  RuntimeInviteMemberResultJsonFn m_inviteMemberJson = nullptr;
  RuntimeRemoveMemberResultJsonFn m_removeMemberWithOpenMlsJson = nullptr;
  RuntimeRemoveMemberResultJsonFn m_removeMemberWithKeyRotationJson = nullptr;
  RuntimeAddChannelMemberResultJsonFn m_addChannelMemberJson = nullptr;
  RuntimeRemoveChannelMemberResultJsonFn m_removeChannelMemberWithOpenMlsJson =
      nullptr;
  RuntimeRemoveChannelMemberResultJsonFn
      m_removeChannelMemberWithKeyRotationJson = nullptr;
  RuntimeExportWorkspaceKeyResultJsonFn m_exportWorkspaceKeyJson = nullptr;
  RuntimeExportTrustSnapshotResultJsonFn m_exportTrustSnapshotJson = nullptr;
  RuntimeRotateWorkspaceManualKeysResultJsonFn m_rotateWorkspaceManualKeysJson =
      nullptr;
  RuntimeRotateWorkspaceForSuspectedCompromiseResultJsonFn
      m_rotateWorkspaceForSuspectedCompromiseJson = nullptr;
  RuntimeDetectCompromiseResultJsonFn m_detectCompromiseJson = nullptr;
  RuntimeOpenMlsWorkspaceActionResultJsonFn m_respondCompromiseJson = nullptr;
  RuntimeImportWorkspaceKeyResultJsonFn m_importWorkspaceKeyJson = nullptr;
  RuntimeExportChannelKeyResultJsonFn m_exportChannelKeyJson = nullptr;
  RuntimeRotateChannelKeyResultJsonFn m_rotateChannelKeyJson = nullptr;
  RuntimeImportChannelKeyResultJsonFn m_importChannelKeyJson = nullptr;
  RuntimeExportRecoveryBundleResultJsonFn m_exportRecoveryBundleJson = nullptr;
  RuntimeImportRecoveryBundleResultJsonFn m_importRecoveryBundleJson = nullptr;
  RuntimeReindexWorkspaceSearchResultJsonFn m_reindexWorkspaceSearchJson =
      nullptr;
  RuntimeSearchWorkspaceResultJsonFn m_searchWorkspaceJson = nullptr;
  RuntimeSearchWorkspaceChannelsResultJsonFn m_searchWorkspaceChannelsJson =
      nullptr;
  RuntimeDirectSyncResultJsonFn m_publishWorkspaceJson = nullptr;
  RuntimeDirectSyncResultJsonFn m_backupWorkspaceJson = nullptr;
  RuntimeDirectEventPublishResultJsonFn m_publishEventWithTrustSnapshotJson =
      nullptr;
  RuntimeDirectSyncResultJsonFn m_pullWorkspaceJson = nullptr;
  RuntimeDirectSyncResultJsonFn m_syncWorkspaceJson = nullptr;
  RuntimeDirectRetryResultJsonFn m_retryBlobTransfersJson = nullptr;
  RuntimeWorkspacePublishQueueResultJsonFn m_workspacePublishQueueJson =
      nullptr;
  RuntimeWorkspaceStorageHealthResultJsonFn m_workspaceStorageHealthJson =
      nullptr;
  RuntimeRepairWorkspaceStorageMetadataResultJsonFn
      m_repairWorkspaceStorageMetadataJson = nullptr;
  RuntimeStartDirectPeerResultJsonFn m_startDirectPeerJson = nullptr;
  RuntimeStartIrohPeerResultJsonFn m_startIrohPeerJson = nullptr;
  RuntimeStopDirectPeerResultJsonFn m_stopDirectPeerJson = nullptr;
  RuntimeSetIdentityPassphraseFn m_setIdentityPassphrase = nullptr;
  RuntimeClearIdentityPassphraseFn m_clearIdentityPassphrase = nullptr;
  FreeStringFn m_freeString = nullptr;
  bool m_ffiReady = false;
  bool m_syncInFlight = false;
  bool m_channelPageInFlight = false;
  bool m_memberPageInFlight = false;
  bool m_peerHostingInFlight = false;
  bool m_keyTransferInFlight = false;
  bool m_autoBackupEnabled = false;
  bool m_rawEventStoreMode = false;
  bool m_runtimeUnlockRequired = false;
  QString m_runtimeDir;
  QString m_identityFile;
  QString m_identityPassphrase;
  bool m_identityPassphraseFromEnvironment = false;
  bool m_runtimeAccessSuspendedUntilUnlock = false;
  QString m_eventStorePath;
  QString m_workspaceId;
  QString m_defaultPeerEndpoint;
  QStringList m_backupPeerEndpoints;
  QVariantMap m_backupPeerStatuses;
  QString m_deviceId;
  QString m_hostedPeerId;
  QString m_hostedPeerEndpoint;
  QString m_hostedPeerEndpointId;
  QString m_hostedPeerTransport;
  QVariantMap m_workspaceSnapshot;
  QVariantMap m_publishQueue;
  QVariantMap m_workspaceStorageHealth;
  QVariantList m_workspaceSummaries;
  QVariantList m_messageSearchHits;
  QString m_messageSearchQuery;
  int m_messageSearchHitCount = 0;
  bool m_messageSearchHasMoreHits = false;
  QVariantList m_channelSearchResults;
  QString m_channelSearchQuery;
  quint64 m_runtimeWriteGeneration = 0;
  quint64 m_lastAppliedRuntimeWriteGeneration = 0;
  quint64 m_workspaceSummariesGeneration = 0;
  quint64 m_publishQueueGeneration = 0;
  quint64 m_workspaceStorageHealthGeneration = 0;
  quint64 m_messageSearchGeneration = 0;
  quint64 m_channelSearchGeneration = 0;
  quint64 m_channelPageGeneration = 0;
  quint64 m_memberPageGeneration = 0;
  quint64 m_timelinePageGeneration = 0;
  quint64 m_readMarkerGeneration = 0;
  QString m_syncStatus;
  QString m_keyTransferJson;
  qsizetype m_nextBackupPeerIndex = 0;
};

bool desktopSmokeFlagEnabled() {
  const auto value = qEnvironmentVariable("CHAFT_DESKTOP_SMOKE")
                         .trimmed()
                         .toLower();
  return value == QStringLiteral("1") || value == QStringLiteral("true") ||
         value == QStringLiteral("yes") || value == QStringLiteral("on");
}

int desktopSmokeTimeoutMs() {
  const auto value =
      qEnvironmentVariable("CHAFT_DESKTOP_SMOKE_TIMEOUT_MS").trimmed();
  bool ok = false;
  const auto parsed = value.toInt(&ok);
  if (!ok) {
    return 15000;
  }
  return qBound(1000, parsed, 60000);
}

bool desktopSmokeSnapshotContainsText(const QVariantMap &snapshot,
                                      const QString &expectedText) {
  if (expectedText.isEmpty()) {
    return !snapshot.value(QStringLiteral("workspaceId")).toString().isEmpty();
  }

  const auto timeline = snapshot.value(QStringLiteral("timeline")).toList();
  for (const auto &itemValue : timeline) {
    const auto item = itemValue.toMap();
    if (item.value(QStringLiteral("body")).toString() == expectedText) {
      return true;
    }
  }
  return false;
}

bool saveDesktopSmokeScreenshot(const QString &path, QString *errorMessage) {
  if (path.isEmpty()) {
    return true;
  }

  QWindow *window = nullptr;
  const auto windows = QGuiApplication::topLevelWindows();
  for (auto *candidate : windows) {
    if (candidate != nullptr && candidate->isVisible()) {
      window = candidate;
      break;
    }
  }
  if (window == nullptr && !windows.isEmpty()) {
    window = windows.first();
  }
  if (window == nullptr) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("no desktop window to capture");
    }
    return false;
  }

  auto *screen = window->screen();
  if (screen == nullptr) {
    screen = QGuiApplication::primaryScreen();
  }
  if (screen == nullptr) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("no screen available for desktop capture");
    }
    return false;
  }

  const QFileInfo fileInfo(path);
  const auto outputDir = fileInfo.absoluteDir();
  if (!outputDir.exists() && !QDir().mkpath(outputDir.absolutePath())) {
    if (errorMessage != nullptr) {
      *errorMessage =
          QStringLiteral("failed to create screenshot directory: %1")
              .arg(outputDir.absolutePath());
    }
    return false;
  }

  const auto pixmap = screen->grabWindow(window->winId());
  if (pixmap.isNull()) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("desktop screenshot capture returned null");
    }
    return false;
  }

  if (!pixmap.save(path, "PNG")) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("failed to write desktop screenshot: %1")
                          .arg(path);
    }
    return false;
  }

  return true;
}

void configureDesktopSmoke(QCoreApplication *app,
                           ChaftController *controller) {
  if (!desktopSmokeFlagEnabled()) {
    return;
  }

  const auto expectedText =
      qEnvironmentVariable("CHAFT_DESKTOP_SMOKE_EXPECT_TEXT").trimmed();
  const auto screenshotPath =
      qEnvironmentVariable("CHAFT_DESKTOP_SMOKE_SCREENSHOT").trimmed();
  const auto timeoutMs = desktopSmokeTimeoutMs();
  const auto completed = std::make_shared<bool>(false);

  const auto checkSnapshot = [app, controller, expectedText, screenshotPath,
                              completed]() {
    if (*completed) {
      return;
    }
    const auto snapshot = controller->workspaceSnapshot();
    if (!desktopSmokeSnapshotContainsText(snapshot, expectedText)) {
      return;
    }

    *completed = true;
    const auto workspaceId =
        snapshot.value(QStringLiteral("workspaceId")).toString().toUtf8();
    if (screenshotPath.isEmpty()) {
      std::fprintf(stderr, "desktop smoke passed: workspace=%s\n",
                   workspaceId.constData());
      finishDesktopSmoke(0);
      return;
    }

    QTimer::singleShot(250, app, [screenshotPath, workspaceId]() {
      QString errorMessage;
      if (!saveDesktopSmokeScreenshot(screenshotPath, &errorMessage)) {
        const auto error = errorMessage.toUtf8();
        std::fprintf(stderr, "desktop smoke screenshot failed: %s\n",
                     error.constData());
        finishDesktopSmoke(125);
        return;
      }

      const auto screenshot = screenshotPath.toUtf8();
      std::fprintf(stderr,
                   "desktop smoke passed: workspace=%s screenshot=%s\n",
                   workspaceId.constData(), screenshot.constData());
      finishDesktopSmoke(0);
    });
  };

  QObject::connect(controller, &ChaftController::workspaceSnapshotChanged, app,
                   checkSnapshot, Qt::QueuedConnection);
  QTimer::singleShot(0, app, checkSnapshot);
  QTimer::singleShot(timeoutMs, app, [controller, expectedText, completed]() {
    if (*completed) {
      return;
    }
    *completed = true;
    const auto snapshot = controller->workspaceSnapshot();
    const auto workspaceId =
        snapshot.value(QStringLiteral("workspaceId")).toString().toUtf8();
    const auto syncStatus = controller->syncStatus().toUtf8();
    const auto expected = expectedText.toUtf8();
    std::fprintf(stderr,
                 "desktop smoke timed out: workspace=%s expected=%s status=%s\n",
                 workspaceId.constData(), expected.constData(),
                 syncStatus.constData());
    finishDesktopSmoke(124);
  });
}

int main(int argc, char *argv[]) {
  if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE")) {
    qputenv("QT_QUICK_CONTROLS_STYLE", "Basic");
  }

  QGuiApplication app(argc, argv);

  ChaftController chaftController(initialWorkspaceSnapshot());
  QQmlApplicationEngine engine;
  engine.rootContext()->setContextProperty("initialWorkspaceSnapshot",
                                           chaftController.workspaceSnapshot());
  engine.rootContext()->setContextProperty("chaftController", &chaftController);
  QObject::connect(
      &engine, &QQmlApplicationEngine::objectCreationFailed, &app,
      []() { QCoreApplication::exit(-1); }, Qt::QueuedConnection);
  addDesktopQmlImportPaths(&engine);
  loadDesktopQml(&engine);
  configureDesktopSmoke(&app, &chaftController);

  return app.exec();
}

#include "main.moc"
