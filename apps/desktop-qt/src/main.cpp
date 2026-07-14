#include <QAbstractSocket>
#include <QApplication>
#include <QByteArray>
#include <QClipboard>
#include <QColor>
#include <QCoreApplication>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QFont>
#include <QFontDatabase>
#include <QGuiApplication>
#include <QHostAddress>
#include <QIODevice>
#include <QImage>
#include <QIcon>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QLibrary>
#include <QLockFile>
#include <QMetaObject>
#include <QObject>
#include <QPointer>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QPixmap>
#include <QQuickItem>
#include <QQuickItemGrabResult>
#include <QQuickWindow>
#include <QSaveFile>
#include <QScreen>
#include <QSharedPointer>
#include <QStandardPaths>
#include <QStringList>
#include <QSystemTrayIcon>
#include <QThread>
#include <QTimer>
#include <QUrl>
#include <QUuid>
#include <QVariant>
#include <QVariantList>
#include <QVariantMap>
#include <QWindow>
#include <QtGlobal>
#include <algorithm>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <functional>
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
using RuntimeCreateWorkspaceWithAccessPolicyResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *);
using RuntimeCreateChannelResultJsonFn = char *(*)(const char *, const char *,
                                                   const char *, const char *,
                                                   bool);
using RuntimeCreateDirectMessageChannelResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *);
using RuntimeUpdateChannelDetailsResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *, const char *);
using RuntimeUpdateChannelArchiveResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *, bool);
using RuntimeUpdateDeviceProfileResultJsonFn = char *(*)(const char *,
                                                         const char *,
                                                         const char *,
                                                         const char *);
using RuntimeUpdateLocalPersonProfileResultJsonFn = char *(*)(const char *,
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
using RuntimeCreateWorkspaceInviteResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *, const char *, const char *, const char *);
using RuntimePrepareWorkspaceInviteClaimResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *, const char *);
using RuntimeWorkspaceInviteEnvelopeResultJsonFn =
    char *(*)(const char *, const char *, const char *);
using RuntimeRecordWorkspaceJoinRequestResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *, const char *, const char *, const char *,
              const char *, const char *, const char *);
using RuntimeRecordWorkspaceJoinRequestWithResponseRouteResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *, const char *, const char *, const char *,
              const char *, const char *, const char *, const char *);
using RuntimeRecordWorkspaceInviteResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *, const char *, const char *, const char *,
              const char *, const char *, const char *);
using RuntimeResolveWorkspaceInviteResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *);
using RuntimeResolveWorkspaceJoinRequestResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *,
              const char *);
using RuntimeUpdateMemberRoleResultJsonFn = char *(*)(const char *,
                                                      const char *,
                                                      const char *,
                                                      const char *,
                                                      const char *);
using RuntimeUpdateWorkspaceAccessPolicyResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *);
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
using RuntimeSubmitJoinRequestDirectResultJsonFn =
    char *(*)(const char *, const char *, const char *);
using RuntimePullJoinAccessDirectResultJsonFn =
    char *(*)(const char *, const char *, const char *, std::size_t);
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
using RuntimeListJoinRequestInboxResultJsonFn = char *(*)(const char *,
                                                          std::size_t);
using RuntimeAckJoinRequestInboxEntryResultJsonFn = char *(*)(const char *,
                                                              const char *);
using RuntimeQueueJoinRequestOutboxResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *);
using RuntimeListJoinRequestOutboxResultJsonFn = char *(*)(const char *,
                                                           std::size_t);
using RuntimeSubmitJoinRequestOutboxEntryDirectResultJsonFn =
    char *(*)(const char *, const char *);
using RuntimeListJoinResponseInboxResultJsonFn = char *(*)(const char *,
                                                           std::size_t);
using RuntimeAckJoinResponseInboxEntryResultJsonFn = char *(*)(const char *,
                                                               const char *);
using RuntimeStageJoinResponseInboxResultJsonFn =
    char *(*)(const char *, const char *, const char *);
using RuntimeQueueJoinResponseOutboxResultJsonFn =
    char *(*)(const char *, const char *, const char *, const char *);
using RuntimeListJoinResponseOutboxResultJsonFn = char *(*)(const char *,
                                                            std::size_t);
using RuntimeSubmitJoinResponseOutboxEntryDirectResultJsonFn =
    char *(*)(const char *, const char *);
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
      *errorMessage = QStringLiteral("local service returned unreadable data");
    }
    return {};
  }

  const auto result = document.object();
  if (!result.value(QStringLiteral("ok")).toBool()) {
    const auto error = result.value(QStringLiteral("error")).toObject();
    const auto message = error.value(QStringLiteral("message"))
                             .toString(QStringLiteral("Action could not finish"));
    if (errorMessage != nullptr) {
      *errorMessage = message;
    }
    return {};
  }

  const auto value = result.value(QStringLiteral("value"));
  if (!value.isObject()) {
    if (errorMessage != nullptr) {
      *errorMessage =
          QStringLiteral("local service returned incomplete data");
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
    return QStringLiteral("ask another admin to review");
  }
  if (reason == QStringLiteral("local_signals_already_handled")) {
    return QStringLiteral("already handled");
  }
  if (reason == QStringLiteral("local_secret_state_missing")) {
    return QStringLiteral("local recovery state missing");
  }
  if (reason == QStringLiteral("no_signals")) {
    return QStringLiteral("no issues");
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
    return QStringLiteral("access refreshed for %2 issue(s), %1 update(s)")
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
    return QStringLiteral("security already handled %1 issue(s)")
        .arg(alreadyHandledCount);
  }

  const auto skippedReason = compromiseSkippedReasonLabel(
      response.value(QStringLiteral("skippedReason")).toString());
  if (!skippedReason.isEmpty()) {
    return QStringLiteral("security review found %1 issue(s), %2")
        .arg(signalCount)
        .arg(skippedReason);
  }

  return QStringLiteral("security reviewed %1 issue(s)").arg(signalCount);
}

QString compromiseReportSummaryText(const QJsonObject &report) {
  const auto signalCount = report.value(QStringLiteral("signalCount")).toInt(0);
  if (signalCount <= 0) {
    return QStringLiteral("security review found no issues");
  }

  QStringList parts;
  parts << QStringLiteral("%1 issue(s)").arg(signalCount);

  const auto localDeviceSignalCount =
      report.value(QStringLiteral("localDeviceSignalCount")).toInt(0);
  if (localDeviceSignalCount > 0) {
    parts << QStringLiteral("%1 on this device").arg(localDeviceSignalCount);
  }

  const auto invalidSignatureCount =
      report.value(QStringLiteral("invalidSignatureCount")).toInt(0);
  if (invalidSignatureCount > 0) {
    parts << QStringLiteral("%1 could not be verified")
                 .arg(invalidSignatureCount);
  }

  if (report.value(QStringLiteral("shouldRotateLocalSecretState"))
          .toBool(false)) {
    parts << QStringLiteral("access refresh recommended");
  } else {
    const auto recommendedAction =
        report.value(QStringLiteral("recommendedAction")).toString();
    if (!recommendedAction.isEmpty()) {
      parts << recommendedAction;
    }
  }

  return QStringLiteral("security review found %1").arg(parts.join(" | "));
}

QString reindexSearchSummaryText(const QJsonObject &report) {
  const auto indexedMessageCount =
      report.value(QStringLiteral("indexedMessageCount")).toInt(0);
  return QStringLiteral("search refreshed for %1 message(s)")
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
      *errorMessage = QStringLiteral("workspace view unavailable");
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
      *errorMessage = QStringLiteral("local service returned no data");
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
      *errorMessage = QStringLiteral("local service returned unreadable data");
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
          QStringLiteral("local service returned incomplete data");
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
      *errorMessage = QStringLiteral("%1 response exceeded %2 bytes")
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
                              QStringLiteral("local service"),
                              errorMessage);
}

QVariantList workspaceSummariesFromRuntime(
    RuntimeListWorkspacesResultJsonFn listFn,
    RuntimeListWorkspacePageResultJsonFn listPageFn, FreeStringFn freeString,
    const QByteArray &runtimeDirBytes, const QByteArray &identityFileBytes,
    QString *errorMessage) {
  if (freeString == nullptr) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("workspace list unavailable");
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
        *errorMessage = QStringLiteral("workspace list unavailable");
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
        "workspaceId": "",
        "name": "",
        "accessPolicy": "invite_only",
        "channels": [],
        "profiles": [],
        "members": [],
        "invites": [],
        "joinRequests": [],
        "keyPackages": [],
        "peerEndpoints": [],
        "recentCommits": [],
        "securityIssues": [],
        "resolvedChannels": {},
        "timeline": [],
        "timelineChannelId": "",
        "timelineWindow": {
            "startIndex": 0,
            "itemCount": 0,
            "totalCount": 0,
            "hasMoreBefore": false,
            "hasMoreAfter": false
        },
        "channelCount": 0,
        "memberCount": 0,
        "inviteCount": 0,
        "joinRequestCount": 0,
        "keyPackageCount": 0,
        "peerEndpointCount": 0,
        "syncStatus": ""
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

constexpr qint64 kMaxDesktopConfigBytes = 256LL * 1024;
constexpr qsizetype kMaxWorkspaceIdBytes = 128;
constexpr qsizetype kMaxThemeIdBytes = 64;
constexpr qsizetype kMaxComposerDrafts = 12;
constexpr qsizetype kMaxComposerDraftKeyBytes = 320;
constexpr qsizetype kMaxComposerDraftBytes = 4096;
constexpr qsizetype kMaxPendingJoinRequests = 5;
constexpr qsizetype kMaxPendingJoinRequestKeyBytes = 160;
constexpr qsizetype kMaxPendingJoinRequestArtifactBytes = 8192;
constexpr qsizetype kMaxWorkspaceInviteArtifacts = 12;
constexpr qsizetype kMaxWorkspaceInviteArtifactKeyBytes = 128;
constexpr qsizetype kMaxWorkspaceInviteArtifactBytes = 8192;
constexpr std::size_t kMaxJoinRequestInboxEntries = 20;
constexpr std::size_t kMaxJoinRequestOutboxEntries = 20;
constexpr qsizetype kMaxJoinRequestOutboxDrainBatch = 3;
constexpr std::size_t kMaxJoinResponseInboxEntries = 20;
constexpr std::size_t kMaxJoinResponseOutboxEntries = 20;
constexpr qsizetype kMaxJoinResponseOutboxDrainBatch = 3;
constexpr std::size_t kMaxAccessEnvelopePullEntries = 20;
constexpr int kJoinRequestInboxPollMs = 5000;
constexpr int kJoinRequestOutboxPollMs = 15000;
constexpr int kJoinResponseInboxPollMs = 5000;
constexpr int kJoinResponseOutboxPollMs = 15000;
constexpr qsizetype kMaxMutedChannels = 128;
constexpr qsizetype kMaxMutedChannelKeyBytes = 320;
constexpr int kMinDesktopWindowWidth = 1040;
constexpr int kMinDesktopWindowHeight = 640;
constexpr int kMaxDesktopWindowWidth = 7680;
constexpr int kMaxDesktopWindowHeight = 4320;

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

QString normalizedThemeId(QString themeId) {
  const auto normalized = themeId.trimmed();
  if (normalized.isEmpty() || normalized.toUtf8().size() > kMaxThemeIdBytes) {
    return {};
  }
  return normalized;
}

QString loadThemeId(const QString &runtimeDir) {
  return normalizedThemeId(loadDesktopConfig(runtimeDir)
                               .value(QStringLiteral("themeId"))
                               .toString());
}

QString normalizedThemeMode(QString themeMode) {
  const auto normalized = themeMode.trimmed().toLower();
  return normalized == QStringLiteral("system") ? normalized
                                                : QStringLiteral("manual");
}

QString loadThemeMode(const QString &runtimeDir) {
  return normalizedThemeMode(loadDesktopConfig(runtimeDir)
                                 .value(QStringLiteral("themeMode"))
                                 .toString());
}

QString loadDarkThemeId(const QString &runtimeDir) {
  return normalizedThemeId(loadDesktopConfig(runtimeDir)
                               .value(QStringLiteral("darkThemeId"))
                               .toString());
}

QString loadLightThemeId(const QString &runtimeDir) {
  return normalizedThemeId(loadDesktopConfig(runtimeDir)
                               .value(QStringLiteral("lightThemeId"))
                               .toString());
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
constexpr qsizetype kMaxChannelTopicBytes = 512;
constexpr qsizetype kMaxChannelIdBytes = 128;
constexpr qsizetype kMaxMessageIdBytes = 128;
constexpr qsizetype kMaxDeviceKeyPackageIdBytes = 128;
constexpr qsizetype kMaxEventIdBytes = 68;
constexpr qsizetype kMaxWorkspaceRoleBytes = 16;
constexpr qsizetype kMaxInviteIdBytes = 128;
constexpr qsizetype kMaxInviteApprovalPolicyBytes = 32;
constexpr qsizetype kMaxWorkspaceAccessPolicyBytes = 32;
constexpr qsizetype kEventIdHashHexBytes = 64;
constexpr qsizetype kMaxDeviceDisplayNameBytes = 128;
constexpr qsizetype kMaxJoinRequestIdBytes = 128;
constexpr qsizetype kMaxJoinRequestNoteBytes = 512;
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
    *error = QStringLiteral("access file not found");
    return false;
  }
  if (!fileInfo.isFile()) {
    *error = QStringLiteral("access file must be a file");
    return false;
  }
  if (fileInfo.size() > kMaxDeviceKeyPackageFileBytes) {
    *error = QStringLiteral("access file is too large (max 64 KB)");
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
        value, QStringLiteral("source message record"), error);
  }

  return validateMetadataTextForWrite(value, kMaxDeviceKeyPackageIdBytes,
                                      QStringLiteral("access record"),
                                      QStringLiteral("128 bytes"), error);
}

bool parseEnabledFlag(const QString &value);

enum class PeerEndpointRoute {
  Unsupported,
  DirectTcp,
  NativeIrohDirect,
  NativeIrohRelay,
  NativeIrohDiscovery
};

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

bool nativeIrohRelayUrlIsValid(const QString &value) {
  if (value.isEmpty() || containsAsciiWhitespace(value)) {
    return false;
  }
  const QUrl url(value);
  return url.isValid() && url.scheme() == QStringLiteral("https") &&
         !url.host().isEmpty();
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
    if (querySeparator == 0) {
      return PeerEndpointRoute::Unsupported;
    }
    const auto endpointId =
        querySeparator < 0 ? rest : rest.left(querySeparator);
    if (!nativeIrohEndpointIdSyntaxIsValid(endpointId)) {
      return PeerEndpointRoute::Unsupported;
    }
    if (querySeparator < 0) {
      return PeerEndpointRoute::NativeIrohDiscovery;
    }
    auto query = rest.mid(querySeparator + 1);
    const auto fragmentSeparator = query.indexOf(QLatin1Char('#'));
    if (fragmentSeparator >= 0) {
      query = query.left(fragmentSeparator);
    }

    auto hasDirectAddr = false;
    auto hasRelay = false;
    for (const auto &parameter :
         query.split(QLatin1Char('&'), Qt::KeepEmptyParts)) {
      const auto equals = parameter.indexOf(QLatin1Char('='));
      const auto key = equals >= 0 ? parameter.left(equals).trimmed()
                                   : parameter.trimmed();
      const auto value =
          equals >= 0 ? parameter.mid(equals + 1).trimmed() : QString();
      if (key == QStringLiteral("relay")) {
        if (!nativeIrohRelayUrlIsValid(value)) {
          return PeerEndpointRoute::Unsupported;
        }
        hasRelay = true;
        continue;
      }
      if (key != QStringLiteral("addr") || !nativeIrohDirectAddrIsValid(value)) {
        return PeerEndpointRoute::Unsupported;
      }
      hasDirectAddr = true;
    }
    if (hasRelay) {
      return PeerEndpointRoute::NativeIrohRelay;
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
  case PeerEndpointRoute::NativeIrohRelay:
    return normalized == QStringLiteral("iroh") ||
           normalized == QStringLiteral("iroh-relay");
  case PeerEndpointRoute::NativeIrohDiscovery:
    return normalized == QStringLiteral("iroh") ||
           normalized == QStringLiteral("iroh-discovery");
  case PeerEndpointRoute::Unsupported:
    return false;
  }
  return false;
}

bool peerEndpointIsSupportedForUse(const QString &endpoint) {
  const auto route = supportedPeerEndpointRoute(endpoint);
  if (route == PeerEndpointRoute::NativeIrohRelay) {
    return parseEnabledFlag(
        qEnvironmentVariable("CHAFT_IROH_ALLOW_PUBLIC_RELAYS"));
  }
  if (route == PeerEndpointRoute::NativeIrohDiscovery) {
    return parseEnabledFlag(
        qEnvironmentVariable("CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY"));
  }
  return route != PeerEndpointRoute::Unsupported;
}

bool validatePeerEndpointForPublish(const QString &endpointId,
                                    const QString &endpoint,
                                    const QString &transport, QString *error) {
  if (!validateMetadataTextForWrite(endpointId, kMaxPeerEndpointIdBytes,
                                    QStringLiteral("address name"),
                                    QStringLiteral("2304 bytes"), error) ||
      !validateMetadataTextForWrite(endpoint, kMaxPeerEndpointBytes,
                                    QStringLiteral("address"),
                                    QStringLiteral("2 KB"), error) ||
      !validateMetadataTextForWrite(transport, kMaxPeerEndpointTransportBytes,
                                    QStringLiteral("address type"),
                                    QStringLiteral("64 bytes"), error)) {
    return false;
  }

  const auto route = supportedPeerEndpointRoute(endpoint);
  if (route == PeerEndpointRoute::Unsupported) {
    *error = QStringLiteral(
        "address is not supported; paste an address from a teammate");
    return false;
  }
  if (!peerEndpointIsSupportedForUse(endpoint)) {
    *error = QStringLiteral("address method is disabled by network policy");
    return false;
  }
  if (!peerEndpointRouteAllowsTransport(route, transport)) {
    *error = QStringLiteral("address method does not match the address");
    return false;
  }
  return true;
}

bool validatePeerEndpointForUse(const QString &endpoint, QString *error) {
  if (!validateMetadataTextForWrite(endpoint, kMaxPeerEndpointBytes,
                                    QStringLiteral("address"),
                                    QStringLiteral("2 KB"), error)) {
    return false;
  }
  if (!peerEndpointIsSupportedForUse(endpoint)) {
    *error = QStringLiteral(
        "address is not supported; paste an address from a teammate");
    return false;
  }
  return true;
}

bool validateDirectListenEndpointForUse(const QString &endpoint,
                                        QString *error) {
  if (!validateMetadataTextForWrite(endpoint, kMaxPeerEndpointBytes,
                                    QStringLiteral("address"),
                                    QStringLiteral("2 KB"), error)) {
    return false;
  }
  if (!directTcpPeerListenAddressIsValid(endpoint)) {
    *error =
        QStringLiteral("custom address must be host:port with numeric port");
    return false;
  }
  return true;
}

bool validatePeerEndpointListForUse(const QStringList &endpoints,
                                    QString *error) {
  if (endpoints.size() > kMaxDirectPeerEndpointListSize) {
    *error = QStringLiteral("address list is too large (max %1)")
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

QString generatedInviteId() {
  return QStringLiteral("inv_") + QUuid::createUuid().toString(QUuid::Id128);
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

QString normalizedInviteApprovalPolicy(const QString &approvalPolicy) {
  const auto normalized = approvalPolicy.trimmed().toLower();
  if (normalized == QStringLiteral("admin_required")) {
    return normalized;
  }
  return QStringLiteral("preapproved");
}

QString normalizedWorkspaceAccessPolicy(const QString &accessPolicy) {
  const auto normalized = accessPolicy.trimmed().toLower();
  if (normalized == QStringLiteral("request_access") ||
      normalized == QStringLiteral("discoverable")) {
    return normalized;
  }
  return QStringLiteral("invite_only");
}

QString inviteSyncExpectation(const QString &peerEndpoint,
                              const QString &approvalPolicy) {
  if (normalizedInviteApprovalPolicy(approvalPolicy) ==
      QStringLiteral("admin_required")) {
    return QStringLiteral("waiting_for_admin_approval");
  }
  if (!peerEndpoint.trimmed().isEmpty()) {
    return QStringLiteral("auto_fetch_from_invite_source");
  }
  return QStringLiteral("needs_reachable_teammate");
}

QString normalizedInviteExpiresAt(const QString &expiresAt,
                                  QString *errorMessage) {
  const auto normalized = expiresAt.trimmed();
  if (normalized.isEmpty()) {
    return {};
  }
  QString metadataError;
  if (!validateMetadataTextForWrite(normalized,
                                    kMaxBackupPeerStatusTimestampBytes,
                                    QStringLiteral("invite expiry"),
                                    QStringLiteral("64 bytes"),
                                    &metadataError)) {
    if (errorMessage != nullptr) {
      *errorMessage = metadataError;
    }
    return {};
  }
  const auto parsed = parseUtcTimestamp(normalized);
  if (!parsed.isValid()) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("invite expiry must be an ISO timestamp");
    }
    return {};
  }
  if (parsed <= QDateTime::currentDateTimeUtc()) {
    if (errorMessage != nullptr) {
      *errorMessage = QStringLiteral("invite expiry must be in the future");
    }
    return {};
  }
  return parsed.toString(Qt::ISODateWithMs);
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

bool backgroundReachabilityEnabled() {
  const auto configured =
      qEnvironmentVariable("CHAFT_DESKTOP_BACKGROUND_REACHABILITY").trimmed();
  if (configured.isEmpty() || configured.toUtf8().size() > 16) {
    return true;
  }
  return parseEnabledFlag(configured);
}

bool developmentLoopbackFallbackEnabled() {
  return parseEnabledFlag(
      qEnvironmentVariable("CHAFT_DESKTOP_ALLOW_LOOPBACK_FALLBACK"));
}

void applyDesktopReachabilityDefaults() {
  if (qEnvironmentVariableIsEmpty("CHAFT_IROH_ALLOW_PUBLIC_RELAYS")) {
    qputenv("CHAFT_IROH_ALLOW_PUBLIC_RELAYS", "1");
  }
  if (qEnvironmentVariableIsEmpty("CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY")) {
    qputenv("CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY", "1");
  }
}

bool loadAutoBackupEnabled(const QString &runtimeDir) {
  return loadDesktopConfig(runtimeDir)
      .value(QStringLiteral("autoBackupEnabled"))
      .toBool(false);
}

bool loadInspectorPinned(const QString &runtimeDir) {
  return loadDesktopConfig(runtimeDir)
      .value(QStringLiteral("inspectorPinned"))
      .toBool(false);
}

bool loadReducedMotionEnabled(const QString &runtimeDir) {
  return loadDesktopConfig(runtimeDir)
      .value(QStringLiteral("reducedMotionEnabled"))
      .toBool(false);
}

bool loadNotificationsEnabled(const QString &runtimeDir) {
  return loadDesktopConfig(runtimeDir)
      .value(QStringLiteral("notificationsEnabled"))
      .toBool(true);
}

bool loadNotificationSoundEnabled(const QString &runtimeDir) {
  return loadDesktopConfig(runtimeDir)
      .value(QStringLiteral("notificationSoundEnabled"))
      .toBool(true);
}

bool loadNotificationPreviewEnabled(const QString &runtimeDir) {
  return loadDesktopConfig(runtimeDir)
      .value(QStringLiteral("notificationPreviewEnabled"))
      .toBool(true);
}

bool loadExternalLinkConfirmationEnabled(const QString &runtimeDir) {
  return loadDesktopConfig(runtimeDir)
      .value(QStringLiteral("externalLinkConfirmationEnabled"))
      .toBool(true);
}

QVariantMap sanitizedMutedChannels(const QVariantMap &mutedChannels) {
  QVariantMap sanitized;
  for (auto it = mutedChannels.constBegin(); it != mutedChannels.constEnd();
       ++it) {
    if (sanitized.size() >= kMaxMutedChannels) {
      break;
    }
    const auto key = it.key().trimmed();
    if (key.isEmpty() || key.toUtf8().size() > kMaxMutedChannelKeyBytes) {
      continue;
    }
    if (!it.value().toBool()) {
      continue;
    }
    sanitized.insert(key, true);
  }
  return sanitized;
}

QVariantMap loadMutedChannels(const QString &runtimeDir) {
  return sanitizedMutedChannels(loadDesktopConfig(runtimeDir)
                                    .value(QStringLiteral("mutedChannels"))
                                    .toObject()
                                    .toVariantMap());
}

QVariantMap sanitizedComposerDrafts(const QVariantMap &drafts) {
  QVariantMap sanitized;
  for (auto it = drafts.constBegin(); it != drafts.constEnd(); ++it) {
    if (sanitized.size() >= kMaxComposerDrafts) {
      break;
    }
    const auto key = it.key().trimmed();
    if (key.isEmpty() || key.toUtf8().size() > kMaxComposerDraftKeyBytes) {
      continue;
    }
    const auto draft = it.value().toString();
    if (draft.trimmed().isEmpty() ||
        draft.toUtf8().size() > kMaxComposerDraftBytes) {
      continue;
    }
    sanitized.insert(key, draft);
  }
  return sanitized;
}

QVariantMap loadComposerDrafts(const QString &runtimeDir) {
  return sanitizedComposerDrafts(loadDesktopConfig(runtimeDir)
                                     .value(QStringLiteral("composerDrafts"))
                                     .toObject()
                                     .toVariantMap());
}

QString sanitizedPendingJoinRequestText(const QVariantMap &request,
                                        const QString &field,
                                        qsizetype maxBytes) {
  const auto value = request.value(field).toString().trimmed();
  if (value.isEmpty() || value.toUtf8().size() > maxBytes) {
    return {};
  }
  return value;
}

QVariantMap sanitizedPendingJoinRequests(const QVariantMap &requests) {
  QVariantMap sanitized;
  for (auto it = requests.constBegin(); it != requests.constEnd(); ++it) {
    if (sanitized.size() >= kMaxPendingJoinRequests) {
      break;
    }
    const auto key = it.key().trimmed();
    if (key.isEmpty() ||
        key.toUtf8().size() > kMaxPendingJoinRequestKeyBytes) {
      continue;
    }
    const auto request = it.value().toMap();
    const auto requestId = sanitizedPendingJoinRequestText(
        request, QStringLiteral("requestId"), kMaxJoinRequestIdBytes);
    const auto workspaceId =
        sanitizedPendingJoinRequestText(request, QStringLiteral("workspaceId"),
                                        kMaxWorkspaceIdBytes);
    const auto workspaceName = sanitizedPendingJoinRequestText(
        request, QStringLiteral("workspaceName"), kMaxWorkspaceNameBytes);
    const auto displayName = sanitizedPendingJoinRequestText(
        request, QStringLiteral("displayName"), kMaxDeviceDisplayNameBytes);
    const auto deliveryDisplayName = sanitizedPendingJoinRequestText(
        request, QStringLiteral("deliveryDisplayName"),
        kMaxDeviceDisplayNameBytes);
    const auto deliveryDeviceId = sanitizedPendingJoinRequestText(
        request, QStringLiteral("deliveryDeviceId"), kMaxDeviceIdReferenceBytes);
    const auto deliveryPeerEndpoint = sanitizedPendingJoinRequestText(
        request, QStringLiteral("deliveryPeerEndpoint"), kMaxPeerEndpointBytes);
    const auto sourceType = sanitizedPendingJoinRequestText(
        request, QStringLiteral("sourceType"), kMaxWorkspaceAccessPolicyBytes);
    const auto sourceInviteId = sanitizedPendingJoinRequestText(
        request, QStringLiteral("sourceInviteId"), kMaxInviteIdBytes);
    const auto sourceDisplayName = sanitizedPendingJoinRequestText(
        request, QStringLiteral("sourceDisplayName"),
        kMaxDeviceDisplayNameBytes);
    const auto sourceApprovalPolicy = sanitizedPendingJoinRequestText(
        request, QStringLiteral("sourceApprovalPolicy"),
        kMaxInviteApprovalPolicyBytes);
    const auto status = sanitizedPendingJoinRequestText(
        request, QStringLiteral("status"), kMaxWorkspaceAccessPolicyBytes);
    const auto createdAt = sanitizedPendingJoinRequestText(
        request, QStringLiteral("createdAt"), kMaxBackupPeerStatusTimestampBytes);
    const auto sentAt = sanitizedPendingJoinRequestText(
        request, QStringLiteral("sentAt"), kMaxBackupPeerStatusTimestampBytes);
    const auto lastAttemptAt = sanitizedPendingJoinRequestText(
        request, QStringLiteral("lastAttemptAt"),
        kMaxBackupPeerStatusTimestampBytes);
    const auto resolvedAt = sanitizedPendingJoinRequestText(
        request, QStringLiteral("resolvedAt"),
        kMaxBackupPeerStatusTimestampBytes);
    const auto error = sanitizedPendingJoinRequestText(
        request, QStringLiteral("error"), kMaxBackupPeerStatusMessageBytes);
    const auto artifact = request.value(QStringLiteral("artifact")).toString();
    if ((workspaceId.isEmpty() && requestId.isEmpty()) ||
        artifact.trimmed().isEmpty() ||
        artifact.toUtf8().size() > kMaxPendingJoinRequestArtifactBytes) {
      continue;
    }

    QVariantMap row;
    if (!requestId.isEmpty()) {
      row.insert(QStringLiteral("requestId"), requestId);
    }
    if (!workspaceId.isEmpty()) {
      row.insert(QStringLiteral("workspaceId"), workspaceId);
    }
    if (!workspaceName.isEmpty()) {
      row.insert(QStringLiteral("workspaceName"), workspaceName);
    }
    if (!displayName.isEmpty()) {
      row.insert(QStringLiteral("displayName"), displayName);
    }
    if (!deliveryDisplayName.isEmpty()) {
      row.insert(QStringLiteral("deliveryDisplayName"), deliveryDisplayName);
    }
    if (!deliveryDeviceId.isEmpty()) {
      row.insert(QStringLiteral("deliveryDeviceId"), deliveryDeviceId);
    }
    if (!deliveryPeerEndpoint.isEmpty()) {
      row.insert(QStringLiteral("deliveryPeerEndpoint"), deliveryPeerEndpoint);
    }
    if (!sourceType.isEmpty()) {
      row.insert(QStringLiteral("sourceType"), sourceType);
    }
    if (!sourceInviteId.isEmpty()) {
      row.insert(QStringLiteral("sourceInviteId"), sourceInviteId);
    }
    if (!sourceDisplayName.isEmpty()) {
      row.insert(QStringLiteral("sourceDisplayName"), sourceDisplayName);
    }
    if (!sourceApprovalPolicy.isEmpty()) {
      row.insert(QStringLiteral("sourceApprovalPolicy"), sourceApprovalPolicy);
    }
    row.insert(QStringLiteral("status"),
               status.isEmpty() ? QStringLiteral("waiting_for_admin")
                                : status);
    if (!createdAt.isEmpty()) {
      row.insert(QStringLiteral("createdAt"), createdAt);
    }
    if (!sentAt.isEmpty()) {
      row.insert(QStringLiteral("sentAt"), sentAt);
    }
    if (!lastAttemptAt.isEmpty()) {
      row.insert(QStringLiteral("lastAttemptAt"), lastAttemptAt);
    }
    if (!resolvedAt.isEmpty()) {
      row.insert(QStringLiteral("resolvedAt"), resolvedAt);
    }
    if (!error.isEmpty()) {
      row.insert(QStringLiteral("error"), error);
    }
    row.insert(QStringLiteral("artifact"), artifact.trimmed());
    sanitized.insert(key, row);
  }
  return sanitized;
}

QVariantMap loadPendingJoinRequests(const QString &runtimeDir) {
  return sanitizedPendingJoinRequests(
      loadDesktopConfig(runtimeDir)
          .value(QStringLiteral("pendingJoinRequests"))
          .toObject()
          .toVariantMap());
}

QVariantMap sanitizedWorkspaceInviteArtifacts(const QVariantMap &artifacts) {
  QVariantMap sanitized;
  for (auto it = artifacts.constBegin(); it != artifacts.constEnd(); ++it) {
    if (sanitized.size() >= kMaxWorkspaceInviteArtifacts) {
      break;
    }
    const auto inviteId = it.key().trimmed();
    const auto artifactText = it.value().toString().trimmed();
    if (inviteId.isEmpty() ||
        inviteId.toUtf8().size() > kMaxWorkspaceInviteArtifactKeyBytes ||
        artifactText.isEmpty() ||
        artifactText.toUtf8().size() > kMaxWorkspaceInviteArtifactBytes) {
      continue;
    }
    QJsonParseError parseError;
    const auto document =
        QJsonDocument::fromJson(artifactText.toUtf8(), &parseError);
    if (parseError.error != QJsonParseError::NoError ||
        !document.isObject()) {
      continue;
    }
    const auto artifact = document.object();
    if (artifact.value(QStringLiteral("kind")).toString() !=
            QStringLiteral("chaft.workspace-invite.v2") ||
        artifact.value(QStringLiteral("inviteId")).toString().trimmed() !=
            inviteId ||
        artifact.value(QStringLiteral("workspaceId"))
            .toString()
            .trimmed()
            .isEmpty() ||
        artifact.value(QStringLiteral("capabilitySecret"))
            .toString()
            .trimmed()
            .isEmpty()) {
      continue;
    }
    sanitized.insert(inviteId, artifactText);
  }
  return sanitized;
}

QVariantMap loadWorkspaceInviteArtifacts(const QString &runtimeDir) {
  return sanitizedWorkspaceInviteArtifacts(
      loadDesktopConfig(runtimeDir)
          .value(QStringLiteral("workspaceInviteArtifacts"))
          .toObject()
          .toVariantMap());
}

QVariantMap sanitizedWindowGeometry(const QVariantMap &geometry) {
  QVariantMap sanitized;
  const auto width = geometry.value(QStringLiteral("width")).toInt(0);
  const auto height = geometry.value(QStringLiteral("height")).toInt(0);
  if (width >= kMinDesktopWindowWidth && width <= kMaxDesktopWindowWidth) {
    sanitized.insert(QStringLiteral("width"), width);
  }
  if (height >= kMinDesktopWindowHeight && height <= kMaxDesktopWindowHeight) {
    sanitized.insert(QStringLiteral("height"), height);
  }
  return sanitized;
}

QVariantMap loadWindowGeometry(const QString &runtimeDir) {
  return sanitizedWindowGeometry(loadDesktopConfig(runtimeDir)
                                     .value(QStringLiteral("windowGeometry"))
                                     .toObject()
                                     .toVariantMap());
}

QString directMessageSlug(QString displayName, const QString &deviceId) {
  auto source = displayName.trimmed().toLower();
  if (source.isEmpty()) {
    source = deviceId.trimmed().left(12).toLower();
  }

  QString slug;
  auto lastDash = false;
  for (const auto &ch : source) {
    if (ch.isLetterOrNumber()) {
      slug.append(ch);
      lastDash = false;
    } else if (!lastDash && !slug.isEmpty()) {
      slug.append(QLatin1Char('-'));
      lastDash = true;
    }
  }
  while (slug.endsWith(QLatin1Char('-'))) {
    slug.chop(1);
  }
  if (slug.isEmpty()) {
    slug = QStringLiteral("person");
  }
  return slug;
}

QString boundedChannelName(QString name) {
  auto bounded = name.trimmed();
  while (bounded.toUtf8().size() > kMaxChannelNameBytes &&
         !bounded.isEmpty()) {
    bounded.chop(1);
  }
  while (bounded.endsWith(QLatin1Char('-'))) {
    bounded.chop(1);
  }
  return bounded.isEmpty() ? QStringLiteral("dm-person") : bounded;
}

QString directMessageChannelName(const QString &displayName,
                                 const QString &deviceId) {
  auto source = displayName.trimmed();
  if (source.isEmpty()) {
    source = QStringLiteral("Unnamed person ") + deviceId.trimmed().left(12);
  }
  return boundedChannelName(source);
}

QString profileDisplayNameForDevice(const QVariantMap &snapshot,
                                    const QString &deviceId) {
  const auto normalizedDeviceId = deviceId.trimmed();
  if (normalizedDeviceId.isEmpty()) {
    return {};
  }
  const auto profiles = snapshot.value(QStringLiteral("profiles")).toList();
  for (const auto &profileValue : profiles) {
    const auto profile = profileValue.toMap();
    if (profile.value(QStringLiteral("deviceId")).toString() ==
        normalizedDeviceId) {
      return profile.value(QStringLiteral("displayName")).toString().trimmed();
    }
  }
  return {};
}

bool exportedWorkspaceKeyLooksValid(const QJsonObject &workspaceKey) {
  return workspaceKey.contains(QStringLiteral("aes256GcmSivKey")) ||
         workspaceKey.contains(QStringLiteral("aes_256_gcm_siv_key"));
}

QByteArray workspaceInvitePackageJson(const QString &workspaceId,
                                      const QString &workspaceName,
                                      const QString &inviteId,
                                      const QString &requestId,
                                      const QString &deviceId,
                                      const QString &inviteeDisplayName,
                                      const QString &role,
                                      const QString &peerEndpoint,
                                      const QString &inviterDeviceId,
                                      const QString &inviterDisplayName,
                                      const QString &expiresAt,
                                      const QString &approvalPolicy,
                                      const QJsonObject &workspaceKey) {
  QJsonObject package;
  package.insert(QStringLiteral("kind"),
                 QStringLiteral("chaft.workspace-invite.v1"));
  package.insert(QStringLiteral("schemaVersion"), 1);
  package.insert(QStringLiteral("workspaceId"), workspaceId);
  package.insert(QStringLiteral("workspaceName"), workspaceName);
  package.insert(QStringLiteral("inviteId"), inviteId);
  if (!requestId.isEmpty()) {
    package.insert(QStringLiteral("requestId"), requestId);
  }
  package.insert(QStringLiteral("inviteeDeviceId"), deviceId);
  if (!inviteeDisplayName.isEmpty()) {
    package.insert(QStringLiteral("inviteeDisplayName"), inviteeDisplayName);
  }
  package.insert(QStringLiteral("role"), role);
  package.insert(QStringLiteral("approvalPolicy"),
                 approvalPolicy.isEmpty() ? QStringLiteral("preapproved")
                                          : approvalPolicy);
  package.insert(QStringLiteral("syncExpectation"),
                 inviteSyncExpectation(peerEndpoint, approvalPolicy));
  package.insert(QStringLiteral("createdAt"), currentUtcTimestamp());
  if (!expiresAt.isEmpty()) {
    package.insert(QStringLiteral("expiresAt"), expiresAt);
  }
  if (!inviterDeviceId.isEmpty()) {
    package.insert(QStringLiteral("inviterDeviceId"), inviterDeviceId);
  }
  if (!inviterDisplayName.isEmpty()) {
    package.insert(QStringLiteral("inviterDisplayName"), inviterDisplayName);
  }
  if (!peerEndpoint.isEmpty()) {
    package.insert(QStringLiteral("peerEndpoint"), peerEndpoint);
  }
  if (!workspaceKey.isEmpty() &&
      normalizedInviteApprovalPolicy(approvalPolicy) !=
          QStringLiteral("admin_required")) {
    package.insert(QStringLiteral("workspaceKey"), workspaceKey);
  }
  return QJsonDocument(package).toJson(QJsonDocument::Indented);
}

QByteArray workspaceJoinRequestJson(const QString &deviceId,
                                    const QString &displayName,
                                    const QString &note,
                                    const QString &workspaceId,
                                    const QString &workspaceName,
                                    const QString &deliveryDeviceId,
                                    const QString &deliveryDisplayName,
                                    const QString &deliveryPeerEndpoint,
                                    const QString &sourceType,
                                    const QString &sourceInviteId,
                                    const QString &sourceDisplayName,
                                    const QString &sourceApprovalPolicy,
                                    const QString &responsePeerEndpoint) {
  QJsonObject request;
  request.insert(QStringLiteral("kind"),
                 QStringLiteral("chaft.workspace-join-request.v1"));
  request.insert(QStringLiteral("schemaVersion"), 1);
  request.insert(QStringLiteral("requestId"),
                 QStringLiteral("req_") +
                     QUuid::createUuid().toString(QUuid::WithoutBraces));
  request.insert(QStringLiteral("deviceId"), deviceId);
  if (!displayName.isEmpty()) {
    request.insert(QStringLiteral("displayName"), displayName);
  }
  if (!note.isEmpty()) {
    request.insert(QStringLiteral("note"), note);
  }
  if (!workspaceId.isEmpty()) {
    request.insert(QStringLiteral("workspaceId"), workspaceId);
  }
  if (!workspaceName.isEmpty()) {
    request.insert(QStringLiteral("workspaceName"), workspaceName);
  }
  if (!deliveryDeviceId.isEmpty()) {
    request.insert(QStringLiteral("deliveryDeviceId"), deliveryDeviceId);
  }
  if (!deliveryDisplayName.isEmpty()) {
    request.insert(QStringLiteral("deliveryDisplayName"), deliveryDisplayName);
  }
  if (!deliveryPeerEndpoint.isEmpty()) {
    request.insert(QStringLiteral("deliveryPeerEndpoint"), deliveryPeerEndpoint);
  }
  if (!sourceType.isEmpty()) {
    request.insert(QStringLiteral("sourceType"), sourceType);
  }
  if (!sourceInviteId.isEmpty()) {
    request.insert(QStringLiteral("sourceInviteId"), sourceInviteId);
  }
  if (!sourceDisplayName.isEmpty()) {
    request.insert(QStringLiteral("sourceDisplayName"), sourceDisplayName);
  }
  if (!sourceApprovalPolicy.isEmpty()) {
    request.insert(QStringLiteral("sourceApprovalPolicy"), sourceApprovalPolicy);
  }
  if (!responsePeerEndpoint.isEmpty()) {
    request.insert(QStringLiteral("responsePeerEndpoint"), responsePeerEndpoint);
  }
  request.insert(QStringLiteral("createdAt"),
                 QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs));
  return QJsonDocument(request).toJson(QJsonDocument::Indented);
}

QByteArray workspaceJoinResponseJson(const QString &workspaceId,
                                     const QString &requestId,
                                     const QString &resolution,
                                     const QString &responderDeviceId,
                                     const QString &responderDisplayName) {
  QJsonObject response;
  response.insert(QStringLiteral("kind"),
                  QStringLiteral("chaft.workspace-join-response.v1"));
  response.insert(QStringLiteral("schemaVersion"), 1);
  response.insert(QStringLiteral("workspaceId"), workspaceId);
  response.insert(QStringLiteral("requestId"), requestId);
  response.insert(QStringLiteral("resolution"), resolution);
  response.insert(QStringLiteral("createdAt"), currentUtcTimestamp());
  if (!responderDeviceId.isEmpty()) {
    response.insert(QStringLiteral("responderDeviceId"), responderDeviceId);
  }
  if (!responderDisplayName.isEmpty()) {
    response.insert(QStringLiteral("responderDisplayName"),
                    responderDisplayName);
  }
  return QJsonDocument(response).toJson(QJsonDocument::Indented);
}

QByteArray workspaceAccessCardJson(const QString &workspaceId,
                                   const QString &workspaceName,
                                   const QString &accessPolicy,
                                   const QString &peerEndpoint,
                                   const QString &adminDeviceId,
                                   const QString &adminDisplayName) {
  QJsonObject card;
  card.insert(QStringLiteral("kind"), QStringLiteral("chaft.workspace-card.v1"));
  card.insert(QStringLiteral("schemaVersion"), 1);
  card.insert(QStringLiteral("workspaceId"), workspaceId);
  card.insert(QStringLiteral("workspaceName"), workspaceName);
  card.insert(QStringLiteral("accessPolicy"),
              normalizedWorkspaceAccessPolicy(accessPolicy));
  card.insert(QStringLiteral("createdAt"),
              QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs));
  if (!peerEndpoint.isEmpty()) {
    card.insert(QStringLiteral("peerEndpoint"), peerEndpoint);
  }
  if (!adminDeviceId.isEmpty()) {
    card.insert(QStringLiteral("adminDeviceId"), adminDeviceId);
  }
  if (!adminDisplayName.isEmpty()) {
    card.insert(QStringLiteral("adminDisplayName"), adminDisplayName);
  }
  return QJsonDocument(card).toJson(QJsonDocument::Indented);
}

QString keyTransferStatusLabel(const QByteArray &bytes) {
  QJsonParseError parseError;
  const auto document = QJsonDocument::fromJson(bytes, &parseError);
  if (parseError.error != QJsonParseError::NoError || !document.isObject()) {
    return QStringLiteral("credentials");
  }
  const auto object = document.object();
  const auto kind = object.value(QStringLiteral("kind")).toString();
  if (kind == QStringLiteral("chaft.workspace-invite.v1") ||
      kind == QStringLiteral("chaft.workspace-invite.v2")) {
    return QStringLiteral("invite");
  }
  if (kind == QStringLiteral("chaft.workspace-invite-claim.v1")) {
    return QStringLiteral("invite claim");
  }
  if (kind == QStringLiteral("chaft.workspace-invite-response.v1")) {
    return QStringLiteral("secure access");
  }
  if (object.value(QStringLiteral("kind")).toString() ==
      QStringLiteral("chaft.workspace-card.v1")) {
    return QStringLiteral("request card");
  }
  if (object.value(QStringLiteral("kind")).toString() ==
      QStringLiteral("chaft.workspace-join-request.v1")) {
    return QStringLiteral("access request");
  }
  if (object.contains(QStringLiteral("schemaVersion")) &&
      object.contains(QStringLiteral("workspaceId")) &&
      object.contains(QStringLiteral("exporterDeviceId")) &&
      object.contains(QStringLiteral("kdf")) &&
      object.contains(QStringLiteral("sealedPayload"))) {
    return QStringLiteral("recovery kit");
  }
  return QStringLiteral("credentials");
}

bool accessApprovalMatchesCurrentHandoff(const QString &currentText,
                                         const QString &approvalText) {
  const auto currentTrimmed = currentText.trimmed();
  const auto approvalTrimmed = approvalText.trimmed();
  if (currentTrimmed.isEmpty() || currentTrimmed == approvalTrimmed) {
    return true;
  }

  QJsonParseError currentError;
  QJsonParseError approvalError;
  const auto currentDocument =
      QJsonDocument::fromJson(currentTrimmed.toUtf8(), &currentError);
  const auto approvalDocument =
      QJsonDocument::fromJson(approvalTrimmed.toUtf8(), &approvalError);
  if (currentError.error != QJsonParseError::NoError ||
      approvalError.error != QJsonParseError::NoError ||
      !currentDocument.isObject() || !approvalDocument.isObject()) {
    return false;
  }

  const auto current = currentDocument.object();
  const auto approval = approvalDocument.object();
  const auto currentKind =
      current.value(QStringLiteral("kind")).toString().trimmed();
  const auto approvalKind =
      approval.value(QStringLiteral("kind")).toString().trimmed();
  if (currentKind != QStringLiteral("chaft.workspace-join-request.v1") &&
      currentKind != QStringLiteral("chaft.workspace-invite-claim.v1")) {
    return false;
  }
  if (approvalKind != QStringLiteral("chaft.workspace-invite.v1") &&
      approvalKind != QStringLiteral("chaft.workspace-invite-response.v1")) {
    return false;
  }

  const auto currentRequestId =
      current.value(QStringLiteral("requestId")).toString().trimmed();
  const auto approvalRequestId =
      approval.value(QStringLiteral("requestId")).toString().trimmed();
  return !currentRequestId.isEmpty() && currentRequestId == approvalRequestId;
}

QIcon desktopNotificationIcon() {
  const auto appIcon = QGuiApplication::windowIcon();
  if (!appIcon.isNull()) {
    return appIcon;
  }

  QPixmap pixmap(32, 32);
  pixmap.fill(QColor(0, 209, 193));
  return QIcon(pixmap);
}

void saveDesktopConfig(const QString &runtimeDir, const QString &workspaceId,
                       const QString &defaultPeerEndpoint,
                       const QStringList &backupPeerEndpoints,
                       const QVariantMap &backupPeerStatuses,
                       bool autoBackupEnabled, const QString &themeId,
                       const QString &themeMode, const QString &darkThemeId,
                       const QString &lightThemeId, bool inspectorPinned,
                       bool reducedMotionEnabled,
                       bool notificationsEnabled,
                       bool notificationSoundEnabled,
                       bool notificationPreviewEnabled,
                       bool externalLinkConfirmationEnabled,
                       const QVariantMap &mutedChannels,
                       const QVariantMap &composerDrafts,
                       const QVariantMap &pendingJoinRequests,
                       const QVariantMap &workspaceInviteArtifacts,
                       const QVariantMap &windowGeometry) {
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
  const auto normalizedTheme = normalizedThemeId(themeId);
  if (!normalizedTheme.isEmpty()) {
    config.insert(QStringLiteral("themeId"), normalizedTheme);
  }
  if (normalizedThemeMode(themeMode) == QStringLiteral("system")) {
    config.insert(QStringLiteral("themeMode"), QStringLiteral("system"));
  }
  const auto normalizedDarkTheme = normalizedThemeId(darkThemeId);
  if (!normalizedDarkTheme.isEmpty()) {
    config.insert(QStringLiteral("darkThemeId"), normalizedDarkTheme);
  }
  const auto normalizedLightTheme = normalizedThemeId(lightThemeId);
  if (!normalizedLightTheme.isEmpty()) {
    config.insert(QStringLiteral("lightThemeId"), normalizedLightTheme);
  }
  if (inspectorPinned) {
    config.insert(QStringLiteral("inspectorPinned"), inspectorPinned);
  }
  if (reducedMotionEnabled) {
    config.insert(QStringLiteral("reducedMotionEnabled"), reducedMotionEnabled);
  }
  if (!notificationsEnabled) {
    config.insert(QStringLiteral("notificationsEnabled"), notificationsEnabled);
  }
  if (!notificationSoundEnabled) {
    config.insert(QStringLiteral("notificationSoundEnabled"),
                  notificationSoundEnabled);
  }
  if (!notificationPreviewEnabled) {
    config.insert(QStringLiteral("notificationPreviewEnabled"),
                  notificationPreviewEnabled);
  }
  if (!externalLinkConfirmationEnabled) {
    config.insert(QStringLiteral("externalLinkConfirmationEnabled"),
                  externalLinkConfirmationEnabled);
  }
  const auto sanitizedMuted = sanitizedMutedChannels(mutedChannels);
  if (!sanitizedMuted.isEmpty()) {
    config.insert(QStringLiteral("mutedChannels"),
                  QJsonObject::fromVariantMap(sanitizedMuted));
  }
  const auto sanitizedDrafts = sanitizedComposerDrafts(composerDrafts);
  if (!sanitizedDrafts.isEmpty()) {
    config.insert(QStringLiteral("composerDrafts"),
                  QJsonObject::fromVariantMap(sanitizedDrafts));
  }
  const auto sanitizedRequests =
      sanitizedPendingJoinRequests(pendingJoinRequests);
  if (!sanitizedRequests.isEmpty()) {
    config.insert(QStringLiteral("pendingJoinRequests"),
                  QJsonObject::fromVariantMap(sanitizedRequests));
  }
  const auto sanitizedInviteArtifacts =
      sanitizedWorkspaceInviteArtifacts(workspaceInviteArtifacts);
  if (!sanitizedInviteArtifacts.isEmpty()) {
    config.insert(QStringLiteral("workspaceInviteArtifacts"),
                  QJsonObject::fromVariantMap(sanitizedInviteArtifacts));
  }
  const auto sanitizedGeometry = sanitizedWindowGeometry(windowGeometry);
  if (!sanitizedGeometry.isEmpty()) {
    config.insert(QStringLiteral("windowGeometry"),
                  QJsonObject::fromVariantMap(sanitizedGeometry));
  }

  const auto bytes = QJsonDocument(config).toJson(QJsonDocument::Indented);
  if (bytes.size() > kMaxDesktopConfigBytes) {
    return;
  }

  QSaveFile file(configPath);
  if (!file.open(QIODevice::WriteOnly)) {
    return;
  }
  file.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
  if (file.write(bytes) != static_cast<qint64>(bytes.size())) {
    file.cancelWriting();
    return;
  }
  if (file.commit()) {
    QFile::setPermissions(configPath,
                          QFileDevice::ReadOwner | QFileDevice::WriteOwner);
  }
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
  Q_PROPERTY(int lastRecoveryImportedChannelCount READ
                 lastRecoveryImportedChannelCount NOTIFY
                     lastRecoveryImportedChannelCountChanged)
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
  Q_PROPERTY(
      QString themeId READ themeId WRITE setThemeId NOTIFY themeIdChanged)
  Q_PROPERTY(QString themeMode READ themeMode WRITE setThemeMode NOTIFY
                 themeModeChanged)
  Q_PROPERTY(QString darkThemeId READ darkThemeId WRITE setDarkThemeId NOTIFY
                 darkThemeIdChanged)
  Q_PROPERTY(QString lightThemeId READ lightThemeId WRITE setLightThemeId
                 NOTIFY lightThemeIdChanged)
  Q_PROPERTY(QString smokeUiState READ smokeUiState CONSTANT)
  Q_PROPERTY(QString instanceLabel READ instanceLabel CONSTANT)
  Q_PROPERTY(bool inspectorPinned READ inspectorPinned WRITE setInspectorPinned
                 NOTIFY inspectorPinnedChanged)
  Q_PROPERTY(bool reducedMotionEnabled READ reducedMotionEnabled WRITE
                 setReducedMotionEnabled NOTIFY reducedMotionEnabledChanged)
  Q_PROPERTY(bool notificationsEnabled READ notificationsEnabled WRITE
                 setNotificationsEnabled NOTIFY notificationSettingsChanged)
  Q_PROPERTY(bool notificationSoundEnabled READ notificationSoundEnabled WRITE
                 setNotificationSoundEnabled NOTIFY notificationSettingsChanged)
  Q_PROPERTY(bool notificationPreviewEnabled READ notificationPreviewEnabled WRITE
                 setNotificationPreviewEnabled NOTIFY notificationSettingsChanged)
  Q_PROPERTY(bool externalLinkConfirmationEnabled READ
                 externalLinkConfirmationEnabled WRITE
                     setExternalLinkConfirmationEnabled NOTIFY
                         externalLinkSettingsChanged)
  Q_PROPERTY(QVariantMap mutedChannels READ mutedChannels NOTIFY
                 mutedChannelsChanged)
  Q_PROPERTY(QVariantMap composerDrafts READ composerDrafts WRITE
                 setComposerDrafts NOTIFY composerDraftsChanged)
  Q_PROPERTY(QVariantMap pendingJoinRequests READ pendingJoinRequests WRITE
                 setPendingJoinRequests NOTIFY pendingJoinRequestsChanged)
  Q_PROPERTY(QVariantMap windowGeometry READ windowGeometry WRITE
                 setWindowGeometry NOTIFY windowGeometryChanged)
  Q_PROPERTY(QString lastCreatedChannelId READ lastCreatedChannelId NOTIFY
                 lastCreatedChannelChanged)
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
  Q_PROPERTY(bool keyTransferFromJoinResponseInbox READ
                 keyTransferFromJoinResponseInbox NOTIFY keyTransferJsonChanged)
  Q_PROPERTY(bool keyTransferInFlight READ keyTransferInFlight NOTIFY
                 keyTransferInFlightChanged)
  Q_PROPERTY(bool joinRequestSubmitInFlight READ joinRequestSubmitInFlight
                 NOTIFY joinRequestSubmitInFlightChanged)
  Q_PROPERTY(bool joinRequestInboxInFlight READ joinRequestInboxInFlight
                 NOTIFY joinRequestInboxInFlightChanged)
  Q_PROPERTY(bool accessEnvelopePullInFlight READ accessEnvelopePullInFlight
                 NOTIFY accessEnvelopePullInFlightChanged)

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
    m_inspectorPinned = loadInspectorPinned(m_runtimeDir);
    m_reducedMotionEnabled = loadReducedMotionEnabled(m_runtimeDir);
    m_notificationsEnabled = loadNotificationsEnabled(m_runtimeDir);
    m_notificationSoundEnabled = loadNotificationSoundEnabled(m_runtimeDir);
    m_notificationPreviewEnabled =
        loadNotificationPreviewEnabled(m_runtimeDir);
    m_externalLinkConfirmationEnabled =
        loadExternalLinkConfirmationEnabled(m_runtimeDir);
    m_mutedChannels = loadMutedChannels(m_runtimeDir);
    m_composerDrafts = loadComposerDrafts(m_runtimeDir);
    m_pendingJoinRequests = loadPendingJoinRequests(m_runtimeDir);
    m_workspaceInviteArtifacts = loadWorkspaceInviteArtifacts(m_runtimeDir);
    m_windowGeometry = loadWindowGeometry(m_runtimeDir);
    const auto configuredAutoBackup = qEnvironmentVariable("CHAFT_AUTO_BACKUP");
    if (!configuredAutoBackup.isEmpty()) {
      m_autoBackupEnabled = parseEnabledFlag(configuredAutoBackup);
    }
    m_themeId = loadThemeId(m_runtimeDir);
    m_themeMode = loadThemeMode(m_runtimeDir);
    m_darkThemeId = loadDarkThemeId(m_runtimeDir);
    m_lightThemeId = loadLightThemeId(m_runtimeDir);
    const auto configuredTheme = qEnvironmentVariable("CHAFT_THEME");
    if (!configuredTheme.isEmpty() &&
        configuredTheme.toUtf8().size() <= kMaxThemeIdBytes) {
      const auto normalizedConfiguredTheme = normalizedThemeId(configuredTheme);
      if (!normalizedConfiguredTheme.isEmpty()) {
        m_themeId = normalizedConfiguredTheme;
        m_themeMode = QStringLiteral("manual");
      }
    }
    loadFfi();
    if (m_ffiReady) {
      if (!m_workspaceId.isEmpty()) {
        applyWorkspaceLoadingSnapshot(m_workspaceId);
      }
      if (m_rawEventStoreMode) {
        setSyncStatus(QStringLiteral("loading messages..."));
        QMetaObject::invokeMethod(
            this, [this]() { queueStoreSnapshotHydration(); },
            Qt::QueuedConnection);
      } else {
        setSyncStatus(QStringLiteral("opening workspace..."));
        QMetaObject::invokeMethod(
            this, [this]() { queueRuntimeHydration(); }, Qt::QueuedConnection);
      }
    }
    if (m_syncStatus.isEmpty()) {
      setSyncStatus(hasRuntimeWorkspace() ? QStringLiteral("messages ready")
                                          : QStringLiteral("no workspace yet"));
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
  int lastRecoveryImportedChannelCount() const {
    return m_lastRecoveryImportedChannelCount;
  }
  QString defaultPeerEndpoint() const { return m_defaultPeerEndpoint; }
  QStringList backupPeerEndpoints() const { return m_backupPeerEndpoints; }
  QVariantMap backupPeerStatuses() const { return m_backupPeerStatuses; }
  QVariantMap publishQueue() const { return m_publishQueue; }
  QVariantMap workspaceStorageHealth() const {
    return m_workspaceStorageHealth;
  }
  bool autoBackupEnabled() const { return m_autoBackupEnabled; }
  bool inspectorPinned() const { return m_inspectorPinned; }
  bool reducedMotionEnabled() const { return m_reducedMotionEnabled; }
  bool notificationsEnabled() const { return m_notificationsEnabled; }
  bool notificationSoundEnabled() const { return m_notificationSoundEnabled; }
  bool notificationPreviewEnabled() const {
    return m_notificationPreviewEnabled;
  }
  bool externalLinkConfirmationEnabled() const {
    return m_externalLinkConfirmationEnabled;
  }
  QVariantMap mutedChannels() const { return m_mutedChannels; }
  QVariantMap composerDrafts() const { return m_composerDrafts; }
  QVariantMap pendingJoinRequests() const { return m_pendingJoinRequests; }
  QVariantMap windowGeometry() const { return m_windowGeometry; }
  QString lastCreatedChannelId() const { return m_lastCreatedChannelId; }
  QString themeId() const { return m_themeId; }
  QString themeMode() const { return m_themeMode; }
  QString darkThemeId() const { return m_darkThemeId; }
  QString lightThemeId() const { return m_lightThemeId; }
  QString smokeUiState() const {
    const auto state = qEnvironmentVariable("CHAFT_SMOKE_UI_STATE");
    if (state.toUtf8().size() > 32) {
      return {};
    }
    return state.trimmed().toLower();
  }
  QString instanceLabel() const {
    const auto label =
        qEnvironmentVariable("CHAFT_DESKTOP_INSTANCE_LABEL").trimmed();
    return label.toUtf8().size() <= 64 ? label : QString();
  }
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
  bool keyTransferFromJoinResponseInbox() const {
    return !m_keyTransferJoinResponseInboxEntryId.trimmed().isEmpty();
  }
  bool keyTransferInFlight() const { return m_keyTransferInFlight; }
  bool joinRequestSubmitInFlight() const { return m_joinRequestSubmitInFlight; }
  bool joinRequestInboxInFlight() const { return m_joinRequestInboxInFlight; }
  bool accessEnvelopePullInFlight() const {
    return m_accessEnvelopePullInFlight;
  }
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

  Q_INVOKABLE bool showDesktopNotification(const QString &title,
                                           const QString &message,
                                           bool playSound) {
    const auto notificationTitle = title.trimmed().left(96);
    const auto notificationMessage = message.trimmed().left(360);
    if (notificationTitle.isEmpty() || notificationMessage.isEmpty()) {
      return false;
    }

    if (playSound) {
      QApplication::beep();
    }
    if (!ensureNotificationTrayIcon()) {
      return playSound;
    }

    m_notificationTrayIcon->showMessage(
        notificationTitle, notificationMessage, QSystemTrayIcon::Information,
        6000);
    return true;
  }

  Q_INVOKABLE QString readCredentialFile(const QString &filePath) {
    const auto normalizedFilePath = filePath.trimmed();
    if (normalizedFilePath.isEmpty()) {
      setSyncStatus(QStringLiteral("credentials file required"));
      return {};
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedFilePath, kMaxFfiPathBytes,
                                      QStringLiteral("credentials file path"),
                                      QStringLiteral("64 KB"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return {};
    }

    const QFileInfo fileInfo(normalizedFilePath);
    if (!fileInfo.exists() || !fileInfo.isFile()) {
      setSyncStatus(QStringLiteral("credentials file not found"));
      return {};
    }
    if (fileInfo.size() > kMaxRecoveryBundleJsonBytes) {
      setSyncStatus(QStringLiteral("credentials file is too large"));
      return {};
    }

    QFile file(normalizedFilePath);
    if (!file.open(QIODevice::ReadOnly)) {
      setSyncStatus(QStringLiteral("failed to open credentials file"));
      return {};
    }
    const auto bytes = file.read(kMaxRecoveryBundleJsonBytes + 1);
    if (bytes.size() > kMaxRecoveryBundleJsonBytes) {
      setSyncStatus(QStringLiteral("credentials file is too large"));
      return {};
    }
    const auto text = QString::fromUtf8(bytes).trimmed();
    if (text.isEmpty()) {
      setSyncStatus(QStringLiteral("credentials file is empty"));
      return {};
    }
    setSyncStatus(QStringLiteral("credentials file loaded"));
    return text;
  }

  Q_INVOKABLE bool saveKeyTransferJson(const QString &outputPath) {
    const auto normalizedOutputPath = outputPath.trimmed();
    if (normalizedOutputPath.isEmpty()) {
      setSyncStatus(QStringLiteral("save location required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedOutputPath, kMaxFfiPathBytes,
                                      QStringLiteral("save location"),
                                      QStringLiteral("64 KB"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    const auto bytes = m_keyTransferJson.toUtf8();
    if (bytes.isEmpty()) {
      setSyncStatus(QStringLiteral("access details unavailable"));
      return false;
    }
    const auto transferLabel = keyTransferStatusLabel(bytes);

    const QFileInfo fileInfo(normalizedOutputPath);
    const auto outputDir = fileInfo.absoluteDir();
    if (!outputDir.exists() && !QDir().mkpath(outputDir.absolutePath())) {
      setSyncStatus(QStringLiteral("failed to create output directory"));
      return false;
    }

    QSaveFile file(normalizedOutputPath);
    if (!file.open(QIODevice::WriteOnly)) {
      setSyncStatus(QStringLiteral("failed to open %1 file").arg(transferLabel));
      return false;
    }
    if (file.write(bytes) != static_cast<qint64>(bytes.size()) ||
        file.write("\n", 1) != 1) {
      file.cancelWriting();
      setSyncStatus(QStringLiteral("failed to write %1 file").arg(transferLabel));
      return false;
    }
    if (!file.commit()) {
      setSyncStatus(QStringLiteral("failed to save %1 file").arg(transferLabel));
      return false;
    }

    setSyncStatus(QStringLiteral("%1 saved").arg(transferLabel));
    return true;
  }

  Q_INVOKABLE bool saveTextFile(const QString &outputPath, const QString &text,
                                const QString &label = QString()) {
    const auto normalizedOutputPath = outputPath.trimmed();
    const auto normalizedText = text.trimmed();
    const auto normalizedLabel = label.trimmed();
    const auto itemLabel =
        normalizedLabel.isEmpty() ? QStringLiteral("file") : normalizedLabel;
    if (normalizedOutputPath.isEmpty()) {
      setSyncStatus(QStringLiteral("save location required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedOutputPath, kMaxFfiPathBytes,
                                      QStringLiteral("save location"),
                                      QStringLiteral("64 KB"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    const auto bytes = normalizedText.toUtf8();
    if (bytes.isEmpty()) {
      setSyncStatus(QStringLiteral("%1 unavailable").arg(itemLabel));
      return false;
    }
    if (bytes.size() > kMaxRecoveryBundleJsonBytes) {
      setSyncStatus(QStringLiteral("%1 is too large").arg(itemLabel));
      return false;
    }

    const QFileInfo fileInfo(normalizedOutputPath);
    const auto outputDir = fileInfo.absoluteDir();
    if (!outputDir.exists() && !QDir().mkpath(outputDir.absolutePath())) {
      setSyncStatus(QStringLiteral("failed to create output directory"));
      return false;
    }

    QSaveFile file(normalizedOutputPath);
    if (!file.open(QIODevice::WriteOnly)) {
      setSyncStatus(QStringLiteral("failed to open %1").arg(itemLabel));
      return false;
    }
    if (file.write(bytes) != static_cast<qint64>(bytes.size()) ||
        file.write("\n", 1) != 1) {
      file.cancelWriting();
      setSyncStatus(QStringLiteral("failed to write %1").arg(itemLabel));
      return false;
    }
    if (!file.commit()) {
      setSyncStatus(QStringLiteral("failed to save %1").arg(itemLabel));
      return false;
    }

    setSyncStatus(QStringLiteral("%1 saved").arg(itemLabel));
    return true;
  }

  Q_INVOKABLE bool updateWorkspaceInvitePeerEndpoint(
      const QString &peerEndpoint) {
    const auto normalizedPeerEndpoint = peerEndpoint.trimmed();
    if (normalizedPeerEndpoint.isEmpty()) {
      setSyncStatus(QStringLiteral("sharing address required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointForUse(normalizedPeerEndpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    QJsonParseError parseError;
    const auto document =
        QJsonDocument::fromJson(m_keyTransferJson.toUtf8(), &parseError);
    if (parseError.error != QJsonParseError::NoError ||
        !document.isObject()) {
      setSyncStatus(QStringLiteral("invite unavailable"));
      return false;
    }

    auto object = document.object();
    if (object.value(QStringLiteral("kind")).toString() !=
        QStringLiteral("chaft.workspace-invite.v1")) {
      setSyncStatus(QStringLiteral("current credentials are not an invite"));
      return false;
    }

    object.insert(QStringLiteral("peerEndpoint"), normalizedPeerEndpoint);
    object.insert(
        QStringLiteral("syncExpectation"),
        inviteSyncExpectation(normalizedPeerEndpoint,
                              object.value(QStringLiteral("approvalPolicy"))
                                  .toString(QStringLiteral("preapproved"))));
    object.insert(QStringLiteral("syncSourceUpdatedAt"),
                  QDateTime::currentDateTimeUtc().toString(Qt::ISODateWithMs));
    const auto bytes = QJsonDocument(object).toJson(QJsonDocument::Indented);
    if (bytes.size() > kMaxKeyTransferJsonBytes) {
      setSyncStatus(QStringLiteral("invite package is too large"));
      return false;
    }

    setKeyTransferJson(QString::fromUtf8(bytes));
    setSyncStatus(QStringLiteral("invite sync source updated"));
    return true;
  }

  Q_INVOKABLE void clearKeyTransferJson() { setKeyTransferJson(QString()); }

  Q_INVOKABLE bool hasWorkspaceInviteArtifact(const QString &inviteId) const {
    const auto normalizedInviteId = inviteId.trimmed();
    return !normalizedInviteId.isEmpty() &&
           m_workspaceInviteArtifacts.contains(normalizedInviteId);
  }

  Q_INVOKABLE bool stageWorkspaceInviteArtifact(const QString &inviteId) {
    const auto normalizedInviteId = inviteId.trimmed();
    const auto artifactText =
        m_workspaceInviteArtifacts.value(normalizedInviteId).toString().trimmed();
    if (normalizedInviteId.isEmpty() || artifactText.isEmpty()) {
      setSyncStatus(QStringLiteral("invite is no longer available on this device"));
      return false;
    }
    QJsonParseError parseError;
    const auto document =
        QJsonDocument::fromJson(artifactText.toUtf8(), &parseError);
    if (parseError.error != QJsonParseError::NoError ||
        !document.isObject() ||
        document.object().value(QStringLiteral("workspaceId"))
                .toString()
                .trimmed() != m_workspaceId.trimmed()) {
      setSyncStatus(QStringLiteral("switch to the invite workspace first"));
      return false;
    }
    setKeyTransferJson(artifactText);
    setSyncStatus(QStringLiteral("invite ready to share"));
    return true;
  }

  Q_INVOKABLE bool acknowledgeCurrentJoinResponseInboxEntry() {
    const auto entryId = m_keyTransferJoinResponseInboxEntryId.trimmed();
    if (entryId.isEmpty()) {
      return false;
    }
    return acknowledgeJoinResponseInboxEntry(entryId, true);
  }

  Q_INVOKABLE bool stageWorkspaceJoinRequest(const QString &displayName,
                                             const QString &note,
                                             const QString &workspaceId,
                                             const QString &workspaceName,
                                             const QString &deliveryDeviceId,
                                             const QString &deliveryDisplayName,
                                             const QString &deliveryPeerEndpoint,
                                             const QString &sourceType,
                                             const QString &sourceInviteId,
                                             const QString &sourceDisplayName,
                                             const QString &sourceApprovalPolicy) {
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    const auto normalizedDeviceId = m_deviceId.trimmed();
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("support code unavailable"));
      return false;
    }
    const auto normalizedDisplayName = displayName.trimmed();
    const auto normalizedNote = note.trimmed();
    const auto normalizedWorkspaceId = workspaceId.trimmed();
    const auto normalizedWorkspaceName = workspaceName.trimmed();
    const auto normalizedDeliveryDeviceId = deliveryDeviceId.trimmed();
    const auto normalizedDeliveryDisplayName = deliveryDisplayName.trimmed();
    const auto normalizedDeliveryPeerEndpoint = deliveryPeerEndpoint.trimmed();
    const auto normalizedSourceType = sourceType.trimmed();
    const auto normalizedSourceInviteId = sourceInviteId.trimmed();
    const auto normalizedSourceDisplayName = sourceDisplayName.trimmed();
    const auto normalizedSourceApprovalPolicy = sourceApprovalPolicy.trimmed();
    const auto normalizedResponsePeerEndpoint = m_hostedPeerEndpoint.trimmed();
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("support code"), QStringLiteral("512 bytes"),
            &metadataError) ||
        (!normalizedDisplayName.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedDisplayName, kMaxDeviceDisplayNameBytes,
             QStringLiteral("display name"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        (!normalizedNote.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedNote, kMaxJoinRequestNoteBytes,
             QStringLiteral("access request note"), QStringLiteral("512 bytes"),
             &metadataError)) ||
        (!normalizedWorkspaceId.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedWorkspaceId, kMaxWorkspaceIdBytes,
             QStringLiteral("workspace ID"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        (!normalizedWorkspaceName.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedWorkspaceName, kMaxWorkspaceNameBytes,
             QStringLiteral("workspace name"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        (!normalizedDeliveryDeviceId.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedDeliveryDeviceId, kMaxDeviceIdReferenceBytes,
             QStringLiteral("admin support code"), QStringLiteral("512 bytes"),
             &metadataError)) ||
        (!normalizedDeliveryDisplayName.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedDeliveryDisplayName, kMaxDeviceDisplayNameBytes,
             QStringLiteral("admin display name"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        (!normalizedDeliveryPeerEndpoint.isEmpty() &&
         !validatePeerEndpointForUse(normalizedDeliveryPeerEndpoint,
                                     &metadataError)) ||
        (!normalizedSourceType.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedSourceType, kMaxWorkspaceAccessPolicyBytes,
             QStringLiteral("request source"), QStringLiteral("32 bytes"),
             &metadataError)) ||
        (!normalizedSourceInviteId.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedSourceInviteId, kMaxInviteIdBytes,
             QStringLiteral("source invite ID"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        (!normalizedSourceDisplayName.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedSourceDisplayName, kMaxDeviceDisplayNameBytes,
             QStringLiteral("source display name"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        (!normalizedSourceApprovalPolicy.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedSourceApprovalPolicy, kMaxInviteApprovalPolicyBytes,
             QStringLiteral("source approval policy"), QStringLiteral("32 bytes"),
             &metadataError)) ||
        (!normalizedResponsePeerEndpoint.isEmpty() &&
         !validatePeerEndpointForUse(normalizedResponsePeerEndpoint,
                                     &metadataError))) {
      setSyncStatus(metadataError);
      return false;
    }

    const auto bytes =
        workspaceJoinRequestJson(normalizedDeviceId, normalizedDisplayName,
                                 normalizedNote, normalizedWorkspaceId,
                                 normalizedWorkspaceName,
                                 normalizedDeliveryDeviceId,
                                 normalizedDeliveryDisplayName,
                                 normalizedDeliveryPeerEndpoint,
                                 normalizedSourceType,
                                 normalizedSourceInviteId,
                                 normalizedSourceDisplayName,
                                 normalizedSourceApprovalPolicy,
                                 normalizedResponsePeerEndpoint);
    if (bytes.size() > kMaxKeyTransferJsonBytes) {
      setSyncStatus(QStringLiteral("access request is too large"));
      return false;
    }
    setKeyTransferJson(QString::fromUtf8(bytes));
    setSyncStatus(QStringLiteral("access request ready"));
    return true;
  }

  Q_INVOKABLE bool submitWorkspaceJoinRequestDirect(
      const QString &peerEndpoint, const QString &workspaceId,
      const QString &requestJson) {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_joinRequestSubmitInFlight) {
      setSyncStatus(QStringLiteral("access request already sending"));
      return false;
    }
    if (m_submitJoinRequestDirectJson == nullptr) {
      setSyncStatus(QStringLiteral("direct access request unavailable"));
      return false;
    }

    const auto normalizedPeerEndpoint = peerEndpoint.trimmed();
    const auto normalizedWorkspaceId = workspaceId.trimmed();
    const auto normalizedRequestJson = requestJson.trimmed();
    QString metadataError;
    if (normalizedPeerEndpoint.isEmpty()) {
      setSyncStatus(QStringLiteral("admin sharing address required"));
      return false;
    }
    if (!validatePeerEndpointForUse(normalizedPeerEndpoint, &metadataError) ||
        (!normalizedWorkspaceId.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedWorkspaceId, kMaxWorkspaceIdBytes,
             QStringLiteral("workspace ID"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        !validateMetadataTextForWrite(
            normalizedRequestJson, kMaxPendingJoinRequestArtifactBytes,
            QStringLiteral("access request"), QStringLiteral("8 KB"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (normalizedRequestJson.isEmpty()) {
      setSyncStatus(QStringLiteral("access request required"));
      return false;
    }

    setSyncStatus(QStringLiteral("sending access request..."));
    runWorkspaceJoinRequestDirectSubmit(normalizedPeerEndpoint,
                                        normalizedWorkspaceId,
                                        normalizedRequestJson);
    return true;
  }

  Q_INVOKABLE bool queueWorkspaceJoinRequestOutbox(
      const QString &peerEndpoint, const QString &workspaceId,
      const QString &requestJson) {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_queueJoinRequestOutboxJson == nullptr) {
      setSyncStatus(QStringLiteral("access request queue unavailable"));
      return false;
    }

    const auto normalizedPeerEndpoint = peerEndpoint.trimmed();
    const auto normalizedWorkspaceId = workspaceId.trimmed();
    const auto normalizedRequestJson = requestJson.trimmed();
    QString metadataError;
    if ((!normalizedPeerEndpoint.isEmpty() &&
         !validatePeerEndpointForUse(normalizedPeerEndpoint, &metadataError)) ||
        (!normalizedWorkspaceId.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedWorkspaceId, kMaxWorkspaceIdBytes,
             QStringLiteral("workspace ID"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        !validateMetadataTextForWrite(
            normalizedRequestJson, kMaxPendingJoinRequestArtifactBytes,
            QStringLiteral("access request"), QStringLiteral("8 KB"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (normalizedRequestJson.isEmpty()) {
      setSyncStatus(QStringLiteral("access request required"));
      return false;
    }

    const auto runtimeDirBytes = m_runtimeDir.toUtf8();
    const auto peerEndpointBytes = normalizedPeerEndpoint.toUtf8();
    const auto workspaceIdBytes = normalizedWorkspaceId.toUtf8();
    const auto requestJsonBytes = normalizedRequestJson.toUtf8();
    QString error;
    const auto json = takeFfiString(
        m_queueJoinRequestOutboxJson(
            runtimeDirBytes.constData(),
            peerEndpointBytes.isEmpty() ? nullptr : peerEndpointBytes.constData(),
            workspaceIdBytes.isEmpty() ? nullptr : workspaceIdBytes.constData(),
            requestJsonBytes.constData()),
        m_freeString, &error);
    const auto value =
        error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
    if (value.isEmpty()) {
      setSyncStatus(error);
      return false;
    }
    queueJoinRequestOutboxDrain();
    return true;
  }

  Q_INVOKABLE bool pullAccessResponsesFromPeer(const QString &peerEndpoint,
                                               const QString &workspaceId) {
    return queueAccessEnvelopePullFromPeer(peerEndpoint, workspaceId, false, true,
                                           true);
  }

  Q_INVOKABLE bool stageWorkspaceAccessCard() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    const auto workspaceId = m_workspaceId.trimmed();
    const auto workspaceName =
        m_workspaceSnapshot.value(QStringLiteral("name")).toString().trimmed();
    auto accessPolicy =
        m_workspaceSnapshot.value(QStringLiteral("accessPolicy")).toString();
    if (accessPolicy.trimmed().isEmpty()) {
      accessPolicy = QStringLiteral("invite_only");
    }
    const auto peerEndpoint = !m_hostedPeerEndpoint.trimmed().isEmpty()
                                  ? m_hostedPeerEndpoint.trimmed()
                                  : m_defaultPeerEndpoint.trimmed();
    const auto adminDeviceId = m_deviceId.trimmed();
    const auto adminDisplayName =
        profileDisplayNameForDevice(m_workspaceSnapshot, adminDeviceId);

    QString metadataError;
    if (!validateMetadataTextForWrite(workspaceId, kMaxWorkspaceIdBytes,
                                      QStringLiteral("workspace ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(workspaceName, kMaxWorkspaceNameBytes,
                                      QStringLiteral("workspace name"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(accessPolicy,
                                      kMaxWorkspaceAccessPolicyBytes,
                                      QStringLiteral("workspace access policy"),
                                      QStringLiteral("32 bytes"),
                                      &metadataError) ||
        (!peerEndpoint.isEmpty() &&
         !validatePeerEndpointForUse(peerEndpoint, &metadataError))) {
      setSyncStatus(metadataError);
      return false;
    }

    const auto bytes = workspaceAccessCardJson(
        workspaceId, workspaceName, accessPolicy, peerEndpoint, adminDeviceId,
        adminDisplayName);
    if (bytes.size() > kMaxKeyTransferJsonBytes) {
      setSyncStatus(QStringLiteral("request card is too large"));
      return false;
    }

    setKeyTransferJson(QString::fromUtf8(bytes));
    setSyncStatus(QStringLiteral("request card ready"));
    return true;
  }

  Q_INVOKABLE bool stageWorkspaceInvitePackage(const QString &deviceId,
                                               const QString &role,
                                               const QString &peerEndpoint,
                                               const QString &inviteeDisplayName,
                                               const QString &expiresAt,
                                               const QString &approvalPolicy) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    const auto normalizedDeviceId = deviceId.trimmed();
    const auto normalizedRole =
        role.trimmed().isEmpty() ? QStringLiteral("member") : role.trimmed();
    const auto normalizedPeerEndpoint = peerEndpoint.trimmed();
    const auto normalizedInviteeDisplayName = inviteeDisplayName.trimmed();
    const auto normalizedApprovalPolicy =
        normalizedInviteApprovalPolicy(approvalPolicy);
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("support code required"));
      return false;
    }
    if (normalizedApprovalPolicy == QStringLiteral("admin_required")) {
      setSyncStatus(
          QStringLiteral("approval invites cannot include workspace access"));
      return false;
    }
    QString metadataError;
    const auto normalizedExpiresAt =
        normalizedInviteExpiresAt(expiresAt, &metadataError);
    if (!metadataError.isEmpty()) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("support code"), QStringLiteral("512 bytes"),
            &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedRole, kMaxWorkspaceRoleBytes,
            QStringLiteral("workspace role"), QStringLiteral("16 bytes"),
            &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedApprovalPolicy, kMaxInviteApprovalPolicyBytes,
            QStringLiteral("invite policy"), QStringLiteral("32 bytes"),
            &metadataError) ||
        (!normalizedInviteeDisplayName.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedInviteeDisplayName, kMaxDeviceDisplayNameBytes,
             QStringLiteral("display name"), QStringLiteral("128 bytes"),
             &metadataError))) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!normalizedPeerEndpoint.isEmpty() &&
        !validatePeerEndpointForUse(normalizedPeerEndpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    QJsonParseError parseError;
    const auto credentialsDocument =
        QJsonDocument::fromJson(m_keyTransferJson.toUtf8(), &parseError);
    if (parseError.error != QJsonParseError::NoError ||
        !credentialsDocument.isObject()) {
      setSyncStatus(QStringLiteral("create workspace access first"));
      return false;
    }
    const auto workspaceKey = credentialsDocument.object();
    if (workspaceKey.value(QStringLiteral("workspaceId")).toString() !=
        m_workspaceId) {
      setSyncStatus(QStringLiteral("workspace access does not match selection"));
      return false;
    }
    if (!exportedWorkspaceKeyLooksValid(workspaceKey)) {
      setSyncStatus(QStringLiteral("workspace access export required"));
      return false;
    }

    const auto inviteId = generatedInviteId();
    const auto bytes = workspaceInvitePackageJson(
        m_workspaceId, m_workspaceSnapshot.value(QStringLiteral("name")).toString(),
        inviteId, QString(), normalizedDeviceId, normalizedInviteeDisplayName, normalizedRole,
        normalizedPeerEndpoint, m_deviceId.trimmed(),
        profileDisplayNameForDevice(m_workspaceSnapshot, m_deviceId),
        normalizedExpiresAt, normalizedApprovalPolicy,
        workspaceKey);
    if (bytes.size() > kMaxKeyTransferJsonBytes) {
      setSyncStatus(QStringLiteral("invite package is too large"));
      return false;
    }
    setKeyTransferJson(QString::fromUtf8(bytes));
    setSyncStatus(QStringLiteral("workspace invite package staged"));
    return true;
  }

  Q_INVOKABLE bool prepareWorkspaceInvitePackage(const QString &deviceId,
                                                 const QString &role,
                                                 const QString &peerEndpoint,
                                                 const QString &inviteeDisplayName,
                                                 const QString &expiresAt,
                                                 const QString &approvalPolicy,
                                                 const QString &requestId,
                                                 const QString &responseDeliveryPeerEndpoint =
                                                     QString()) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }

    const auto normalizedDeviceId = deviceId.trimmed();
    const auto normalizedRole =
        role.trimmed().isEmpty() ? QStringLiteral("member") : role.trimmed();
    const auto normalizedPeerEndpoint = peerEndpoint.trimmed();
    const auto normalizedInviteeDisplayName = inviteeDisplayName.trimmed();
    const auto normalizedApprovalPolicy =
        normalizedInviteApprovalPolicy(approvalPolicy);
    const auto normalizedRequestId = requestId.trimmed();
    const auto normalizedResponseDeliveryPeerEndpoint =
        responseDeliveryPeerEndpoint.trimmed();
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("support code required"));
      return false;
    }
    if (normalizedApprovalPolicy == QStringLiteral("admin_required")) {
      setSyncStatus(
          QStringLiteral("approval invites cannot include workspace access"));
      return false;
    }
    QString metadataError;
    const auto normalizedExpiresAt =
        normalizedInviteExpiresAt(expiresAt, &metadataError);
    if (!metadataError.isEmpty()) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("support code"), QStringLiteral("512 bytes"),
            &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedRole, kMaxWorkspaceRoleBytes,
            QStringLiteral("workspace role"), QStringLiteral("16 bytes"),
            &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedApprovalPolicy, kMaxInviteApprovalPolicyBytes,
            QStringLiteral("invite policy"), QStringLiteral("32 bytes"),
            &metadataError) ||
        (!normalizedRequestId.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedRequestId, kMaxJoinRequestIdBytes,
             QStringLiteral("access request ID"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        (!normalizedInviteeDisplayName.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedInviteeDisplayName, kMaxDeviceDisplayNameBytes,
             QStringLiteral("display name"), QStringLiteral("128 bytes"),
             &metadataError))) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!normalizedPeerEndpoint.isEmpty() &&
        !validatePeerEndpointForUse(normalizedPeerEndpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!normalizedResponseDeliveryPeerEndpoint.isEmpty() &&
        !validatePeerEndpointForUse(normalizedResponseDeliveryPeerEndpoint,
                                    &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_inviteMemberJson == nullptr || m_exportWorkspaceKeyJson == nullptr ||
        m_recordWorkspaceInviteJson == nullptr) {
      setSyncStatus(QStringLiteral("invite package unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setKeyTransferJson(QString());
    setKeyTransferInFlight(true);
    setSyncStatus(QStringLiteral("creating invite..."));
    runWorkspaceInvitePackage(normalizedDeviceId, normalizedRole,
                              normalizedPeerEndpoint,
                              normalizedInviteeDisplayName,
                              normalizedExpiresAt, normalizedApprovalPolicy,
                              normalizedRequestId,
                              normalizedResponseDeliveryPeerEndpoint,
                              generation);
    return true;
  }

  Q_INVOKABLE bool prepareClaimableWorkspaceInvite(
      const QString &displayName, const QString &role,
      const QString &peerEndpoint, const QString &expiresAt) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    if (m_createWorkspaceInviteJson == nullptr) {
      setSyncStatus(QStringLiteral("secure invites unavailable"));
      return false;
    }

    const auto normalizedDisplayName = displayName.trimmed();
    const auto normalizedRole =
        role.trimmed().isEmpty() ? QStringLiteral("member") : role.trimmed();
    const auto normalizedPeerEndpoint = peerEndpoint.trimmed();
    QString metadataError;
    const auto normalizedExpiresAt =
        normalizedInviteExpiresAt(expiresAt, &metadataError);
    if (!metadataError.isEmpty()) {
      setSyncStatus(metadataError);
      return false;
    }
    if ((!normalizedDisplayName.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedDisplayName, kMaxDeviceDisplayNameBytes,
             QStringLiteral("invite label"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        !validateMetadataTextForWrite(
            normalizedRole, kMaxWorkspaceRoleBytes,
            QStringLiteral("workspace role"), QStringLiteral("16 bytes"),
            &metadataError) ||
        (!normalizedPeerEndpoint.isEmpty() &&
         !validatePeerEndpointForUse(normalizedPeerEndpoint, &metadataError))) {
      setSyncStatus(metadataError);
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setKeyTransferJson(QString());
    setKeyTransferInFlight(true);
    setSyncStatus(QStringLiteral("creating secure invite..."));
    runClaimableWorkspaceInvite(normalizedDisplayName, normalizedRole,
                                normalizedPeerEndpoint, normalizedExpiresAt,
                                generation);
    return true;
  }

  Q_INVOKABLE bool prepareWorkspaceInviteClaim(
      const QString &artifactJson, const QString &displayName,
      const QString &note) {
    if (!ensureFfiReady() || m_prepareWorkspaceInviteClaimJson == nullptr) {
      setSyncStatus(QStringLiteral("secure invite claims unavailable"));
      return false;
    }
    const auto normalizedArtifactJson = artifactJson.trimmed();
    if (normalizedArtifactJson.isEmpty()) {
      setSyncStatus(QStringLiteral("invite required"));
      return false;
    }
    const auto runtimeDirBytes = m_runtimeDir.toUtf8();
    const auto identityFileBytes = m_identityFile.toUtf8();
    const auto artifactBytes = normalizedArtifactJson.toUtf8();
    const auto displayNameBytes = displayName.trimmed().toUtf8();
    const auto noteBytes = note.trimmed().toUtf8();
    const auto responsePeerEndpointBytes = m_hostedPeerEndpoint.trimmed().toUtf8();
    QString error;
    const auto resultJson = takeFfiString(
        m_prepareWorkspaceInviteClaimJson(
            runtimeDirBytes.constData(),
            identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
            artifactBytes.constData(), displayNameBytes.constData(),
            noteBytes.constData(), responsePeerEndpointBytes.constData()),
        m_freeString, &error);
    const auto value =
        error.isEmpty() ? resultValueFromJson(resultJson, &error) : QJsonObject();
    if (value.isEmpty()) {
      setSyncStatus(error);
      return false;
    }
    const auto bytes = QJsonDocument(value).toJson(QJsonDocument::Indented);
    if (bytes.size() > kMaxKeyTransferJsonBytes) {
      setSyncStatus(QStringLiteral("invite claim is too large"));
      return false;
    }
    setKeyTransferJson(QString::fromUtf8(bytes));
    setSyncStatus(QStringLiteral("invite claim ready"));
    return true;
  }

  Q_INVOKABLE bool importWorkspaceInviteResponse(const QString &responseJson) {
    if (!ensureFfiReady() || m_importWorkspaceInviteResponseJson == nullptr) {
      setSyncStatus(QStringLiteral("secure invite response unavailable"));
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    const auto normalizedResponseJson = responseJson.trimmed();
    if (normalizedResponseJson.isEmpty()) {
      setSyncStatus(QStringLiteral("invite response required"));
      return false;
    }
    const auto previousWorkspaceId = m_workspaceId;
    const auto hadRuntimeWorkspace = hasRuntimeWorkspace();
    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("importing secure workspace access..."));
    runWorkspaceKeyImport(m_importWorkspaceInviteResponseJson,
                          normalizedResponseJson, previousWorkspaceId,
                          hadRuntimeWorkspace, generation,
                          QStringLiteral("workspace access imported"));
    return true;
  }

  Q_INVOKABLE bool prepareApprovalInvitePackage(const QString &deviceId,
                                                const QString &role,
                                                const QString &peerEndpoint,
                                                const QString &inviteeDisplayName,
                                                const QString &expiresAt) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }

    const auto normalizedDeviceId = deviceId.trimmed();
    const auto normalizedRole =
        role.trimmed().isEmpty() ? QStringLiteral("member") : role.trimmed();
    const auto normalizedPeerEndpoint = peerEndpoint.trimmed();
    const auto normalizedInviteeDisplayName = inviteeDisplayName.trimmed();
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("support code required"));
      return false;
    }
    QString metadataError;
    const auto normalizedExpiresAt =
        normalizedInviteExpiresAt(expiresAt, &metadataError);
    if (!metadataError.isEmpty()) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("support code"), QStringLiteral("512 bytes"),
            &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedRole, kMaxWorkspaceRoleBytes,
            QStringLiteral("workspace role"), QStringLiteral("16 bytes"),
            &metadataError) ||
        (!normalizedInviteeDisplayName.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedInviteeDisplayName, kMaxDeviceDisplayNameBytes,
             QStringLiteral("display name"), QStringLiteral("128 bytes"),
             &metadataError))) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!normalizedPeerEndpoint.isEmpty() &&
        !validatePeerEndpointForUse(normalizedPeerEndpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_recordWorkspaceInviteJson == nullptr) {
      setSyncStatus(QStringLiteral("approval invite unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setKeyTransferJson(QString());
    setKeyTransferInFlight(true);
    setSyncStatus(QStringLiteral("creating approval invite..."));
    runWorkspaceApprovalInvitePackage(normalizedDeviceId, normalizedRole,
                                      normalizedPeerEndpoint,
                                      normalizedInviteeDisplayName,
                                      normalizedExpiresAt, generation);
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

    setSyncStatus(QStringLiteral("unlocking workspace..."));
    queueRuntimeHydration();
    return true;
  }

  Q_INVOKABLE void requestRuntimeUnlock() {
    if (!hasRuntimeWorkspace()) {
      return;
    }
    if (m_identityPassphraseFromEnvironment) {
      setSyncStatus(
          QStringLiteral("workspace unlock is provided by environment"));
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
          QStringLiteral("workspace unlock is provided by environment"));
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
    setSyncStatus(QStringLiteral("workspace locked"));
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

  void setThemeId(const QString &themeId) {
    const auto normalized = normalizedThemeId(themeId);
    if (normalized.isEmpty() || m_themeId == normalized) {
      return;
    }
    m_themeId = normalized;
    persistDesktopConfig();
    emit themeIdChanged();
  }

  void setInspectorPinned(bool inspectorPinned) {
    if (m_inspectorPinned == inspectorPinned) {
      return;
    }
    m_inspectorPinned = inspectorPinned;
    persistDesktopConfig();
    emit inspectorPinnedChanged();
  }

  void setReducedMotionEnabled(bool reducedMotionEnabled) {
    if (m_reducedMotionEnabled == reducedMotionEnabled) {
      return;
    }
    m_reducedMotionEnabled = reducedMotionEnabled;
    persistDesktopConfig();
    emit reducedMotionEnabledChanged();
  }

  void setNotificationsEnabled(bool notificationsEnabled) {
    if (m_notificationsEnabled == notificationsEnabled) {
      return;
    }
    m_notificationsEnabled = notificationsEnabled;
    persistDesktopConfig();
    emit notificationSettingsChanged();
  }

  void setNotificationSoundEnabled(bool notificationSoundEnabled) {
    if (m_notificationSoundEnabled == notificationSoundEnabled) {
      return;
    }
    m_notificationSoundEnabled = notificationSoundEnabled;
    persistDesktopConfig();
    emit notificationSettingsChanged();
  }

  void setNotificationPreviewEnabled(bool notificationPreviewEnabled) {
    if (m_notificationPreviewEnabled == notificationPreviewEnabled) {
      return;
    }
    m_notificationPreviewEnabled = notificationPreviewEnabled;
    persistDesktopConfig();
    emit notificationSettingsChanged();
  }

  Q_INVOKABLE bool openStartupSettings() {
#if defined(Q_OS_MACOS)
    const QUrl settingsUrl(QStringLiteral(
        "x-apple.systempreferences:com.apple.LoginItems-Settings.extension"));
#elif defined(Q_OS_WIN)
    const QUrl settingsUrl(QStringLiteral("ms-settings:startupapps"));
#else
    setSyncStatus(QStringLiteral("add Chaft to your system startup apps"));
    return false;
#endif
    if (!QDesktopServices::openUrl(settingsUrl)) {
      setSyncStatus(QStringLiteral("open your system startup apps"));
      return false;
    }
    setSyncStatus(QStringLiteral("startup settings opened"));
    return true;
  }

  void setExternalLinkConfirmationEnabled(
      bool externalLinkConfirmationEnabled) {
    if (m_externalLinkConfirmationEnabled ==
        externalLinkConfirmationEnabled) {
      return;
    }
    m_externalLinkConfirmationEnabled = externalLinkConfirmationEnabled;
    persistDesktopConfig();
    emit externalLinkSettingsChanged();
  }

  Q_INVOKABLE bool setChannelMuted(const QString &workspaceId,
                                   const QString &channelId, bool muted) {
    const auto normalizedWorkspaceId = workspaceId.trimmed();
    const auto normalizedChannelId = channelId.trimmed();
    if (normalizedWorkspaceId.isEmpty() || normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }

    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedWorkspaceId,
                                      kMaxWorkspaceIdBytes,
                                      QStringLiteral("workspace ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("room ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    const auto mutedKey = normalizedWorkspaceId + QStringLiteral("::") +
                          normalizedChannelId;
    if (!validateMetadataTextForWrite(mutedKey, kMaxMutedChannelKeyBytes,
                                      QStringLiteral("room preference"),
                                      QStringLiteral("320 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    auto nextMutedChannels = m_mutedChannels;
    const auto alreadyMuted = nextMutedChannels.value(mutedKey).toBool();
    if (muted == alreadyMuted) {
      return true;
    }
    if (muted && !alreadyMuted &&
        nextMutedChannels.size() >= kMaxMutedChannels) {
      setSyncStatus(QStringLiteral("too many muted rooms"));
      return false;
    }
    if (muted) {
      nextMutedChannels.insert(mutedKey, true);
    } else {
      nextMutedChannels.remove(mutedKey);
    }

    m_mutedChannels = sanitizedMutedChannels(nextMutedChannels);
    persistDesktopConfig();
    emit mutedChannelsChanged();
    setSyncStatus(muted ? QStringLiteral("room muted")
                        : QStringLiteral("room unmuted"));
    return true;
  }

  void setComposerDrafts(const QVariantMap &composerDrafts) {
    const auto sanitized = sanitizedComposerDrafts(composerDrafts);
    if (m_composerDrafts == sanitized) {
      return;
    }
    m_composerDrafts = sanitized;
    persistDesktopConfig();
    emit composerDraftsChanged();
  }

  void setPendingJoinRequests(const QVariantMap &pendingJoinRequests) {
    const auto sanitized = sanitizedPendingJoinRequests(pendingJoinRequests);
    if (m_pendingJoinRequests == sanitized) {
      return;
    }
    m_pendingJoinRequests = sanitized;
    persistDesktopConfig();
    emit pendingJoinRequestsChanged();
  }

  bool rememberWorkspaceInviteArtifact(const QString &artifactText) {
    const auto normalizedArtifactText = artifactText.trimmed();
    if (normalizedArtifactText.isEmpty() ||
        normalizedArtifactText.toUtf8().size() >
            kMaxWorkspaceInviteArtifactBytes) {
      return false;
    }
    QJsonParseError parseError;
    const auto document =
        QJsonDocument::fromJson(normalizedArtifactText.toUtf8(), &parseError);
    if (parseError.error != QJsonParseError::NoError ||
        !document.isObject()) {
      return false;
    }
    const auto artifact = document.object();
    const auto inviteId =
        artifact.value(QStringLiteral("inviteId")).toString().trimmed();
    if (artifact.value(QStringLiteral("kind")).toString() !=
            QStringLiteral("chaft.workspace-invite.v2") ||
        inviteId.isEmpty()) {
      return false;
    }

    auto next = m_workspaceInviteArtifacts;
    if (!next.contains(inviteId) &&
        next.size() >= kMaxWorkspaceInviteArtifacts) {
      QString oldestKey;
      QString oldestCreatedAt;
      for (auto it = next.constBegin(); it != next.constEnd(); ++it) {
        QJsonParseError candidateError;
        const auto candidate = QJsonDocument::fromJson(
            it.value().toString().toUtf8(), &candidateError);
        const auto createdAt =
            candidateError.error == QJsonParseError::NoError &&
                    candidate.isObject()
                ? candidate.object()
                      .value(QStringLiteral("createdAt"))
                      .toString()
                : QString();
        if (oldestKey.isEmpty() || createdAt < oldestCreatedAt) {
          oldestKey = it.key();
          oldestCreatedAt = createdAt;
        }
      }
      next.remove(oldestKey);
    }
    next.insert(inviteId, normalizedArtifactText);
    m_workspaceInviteArtifacts = sanitizedWorkspaceInviteArtifacts(next);
    if (!m_workspaceInviteArtifacts.contains(inviteId)) {
      return false;
    }
    persistDesktopConfig();
    return true;
  }

  void pruneWorkspaceInviteArtifacts(const QVariantMap &snapshot) {
    if (m_workspaceInviteArtifacts.isEmpty()) {
      return;
    }
    auto next = m_workspaceInviteArtifacts;
    const auto now = QDateTime::currentDateTimeUtc();
    for (const auto &inviteValue :
         snapshot.value(QStringLiteral("invites")).toList()) {
      const auto invite = inviteValue.toMap();
      const auto inviteId =
          invite.value(QStringLiteral("inviteId")).toString().trimmed();
      if (inviteId.isEmpty() || !next.contains(inviteId)) {
        continue;
      }
      const auto status =
          invite.value(QStringLiteral("status")).toString().trimmed();
      const auto expiresAt = QDateTime::fromString(
          invite.value(QStringLiteral("expiresAt")).toString(), Qt::ISODate);
      const auto expired = expiresAt.isValid() && expiresAt.toUTC() <= now;
      if (status == QStringLiteral("accepted") ||
          status == QStringLiteral("revoked") || expired) {
        next.remove(inviteId);
      }
    }
    if (next == m_workspaceInviteArtifacts) {
      return;
    }
    m_workspaceInviteArtifacts = sanitizedWorkspaceInviteArtifacts(next);
    persistDesktopConfig();
  }

  bool applyPendingJoinRequestResponse(const QString &requestId,
                                       const QString &workspaceId,
                                       const QString &status,
                                       const QString &message,
                                       const QString &resolvedAt) {
    const auto normalizedRequestId = requestId.trimmed();
    const auto normalizedWorkspaceId = workspaceId.trimmed();
    const auto normalizedStatus = status.trimmed();
    if (normalizedStatus.isEmpty() ||
        (normalizedRequestId.isEmpty() && normalizedWorkspaceId.isEmpty())) {
      return false;
    }

    QString key;
    if (!normalizedWorkspaceId.isEmpty() &&
        m_pendingJoinRequests.contains(normalizedWorkspaceId)) {
      key = normalizedWorkspaceId;
    } else if (!normalizedRequestId.isEmpty() &&
               m_pendingJoinRequests.contains(normalizedRequestId)) {
      key = normalizedRequestId;
    } else {
      for (auto it = m_pendingJoinRequests.constBegin();
           it != m_pendingJoinRequests.constEnd(); ++it) {
        const auto row = it.value().toMap();
        if ((!normalizedRequestId.isEmpty() &&
             row.value(QStringLiteral("requestId")).toString().trimmed() ==
                 normalizedRequestId) ||
            (!normalizedWorkspaceId.isEmpty() &&
             row.value(QStringLiteral("workspaceId")).toString().trimmed() ==
                 normalizedWorkspaceId)) {
          key = it.key();
          break;
        }
      }
    }
    if (key.isEmpty()) {
      return false;
    }

    auto next = m_pendingJoinRequests;
    auto row = next.value(key).toMap();
    const auto normalizedMessage = message.trimmed();
    const auto normalizedResolvedAt =
        resolvedAt.trimmed().isEmpty() ? currentUtcTimestamp()
                                       : resolvedAt.trimmed();
    const auto oldStatus = row.value(QStringLiteral("status")).toString();
    const auto oldMessage = row.value(QStringLiteral("error")).toString();
    const auto oldResolvedAt =
        row.value(QStringLiteral("resolvedAt")).toString();
    if (oldStatus == normalizedStatus && oldMessage == normalizedMessage &&
        oldResolvedAt == normalizedResolvedAt) {
      return false;
    }

    row.insert(QStringLiteral("status"), normalizedStatus);
    row.insert(QStringLiteral("resolvedAt"), normalizedResolvedAt);
    if (normalizedMessage.isEmpty()) {
      row.remove(QStringLiteral("error"));
    } else {
      row.insert(QStringLiteral("error"), normalizedMessage);
    }
    next.insert(key, row);
    setPendingJoinRequests(next);
    return true;
  }

  void setWindowGeometry(const QVariantMap &windowGeometry) {
    const auto sanitized = sanitizedWindowGeometry(windowGeometry);
    if (m_windowGeometry == sanitized) {
      return;
    }
    m_windowGeometry = sanitized;
    persistDesktopConfig();
    emit windowGeometryChanged();
  }

  void setThemeMode(const QString &themeMode) {
    const auto normalized = normalizedThemeMode(themeMode);
    if (m_themeMode == normalized) {
      return;
    }
    m_themeMode = normalized;
    persistDesktopConfig();
    emit themeModeChanged();
  }

  void setDarkThemeId(const QString &darkThemeId) {
    const auto normalized = normalizedThemeId(darkThemeId);
    if (normalized.isEmpty() || m_darkThemeId == normalized) {
      return;
    }
    m_darkThemeId = normalized;
    persistDesktopConfig();
    emit darkThemeIdChanged();
  }

  void setLightThemeId(const QString &lightThemeId) {
    const auto normalized = normalizedThemeId(lightThemeId);
    if (normalized.isEmpty() || m_lightThemeId == normalized) {
      return;
    }
    m_lightThemeId = normalized;
    persistDesktopConfig();
    emit lightThemeIdChanged();
  }

  Q_INVOKABLE bool addBackupPeerEndpoint(const QString &peerEndpoint) {
    const auto normalized = peerEndpoint.trimmed();
    if (normalized.isEmpty()) {
      setSyncStatus(QStringLiteral("backup device required"));
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
      setSyncStatus(QStringLiteral("backup device already saved"));
      return true;
    }
    if (m_backupPeerEndpoints.size() >= kMaxSavedBackupPeerEndpoints) {
      setSyncStatus(
          QStringLiteral("backup device limit reached (%1)")
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
          QStringLiteral("backup device saved and announced"), generation,
          QStringLiteral("full_history_with_blobs"),
          QStringLiteral("operator_saved"));
    } else if (m_runtimeAccessSuspendedUntilUnlock || m_runtimeUnlockRequired) {
      setSyncStatus(QStringLiteral("backup device saved; unlock to announce"));
    } else {
      setSyncStatus(QStringLiteral("backup device saved"));
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
    setSyncStatus(QStringLiteral("backup device removed"));
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
                                   const QString &defaultChannelName,
                                   const QString &accessPolicy) {
    if (!ensureRuntimeAccessReady()) {
      return false;
    }

    const auto workspaceName =
        name.trimmed().isEmpty() ? QStringLiteral("Chaft") : name.trimmed();
    const auto channelName = defaultChannelName.trimmed().isEmpty()
                                 ? QStringLiteral("general")
                                 : defaultChannelName.trimmed();
    const auto normalizedAccessPolicy =
        normalizedWorkspaceAccessPolicy(accessPolicy);
    QString metadataError;
    if (!validateMetadataTextForWrite(workspaceName, kMaxWorkspaceNameBytes,
                                      QStringLiteral("workspace name"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(
            channelName, kMaxChannelNameBytes, QStringLiteral("room name"),
            QStringLiteral("128 bytes"), &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedAccessPolicy, kMaxWorkspaceAccessPolicyBytes,
            QStringLiteral("workspace access policy"), QStringLiteral("32 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_createWorkspaceWithAccessPolicyJson == nullptr) {
      setSyncStatus(QStringLiteral("workspace creation unavailable"));
      return false;
    }

    const auto hadRuntimeWorkspace = hasRuntimeWorkspace();
    const auto previousWorkspaceId = m_workspaceId;
    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("creating workspace..."));
    runWorkspaceCreate(workspaceName, channelName, normalizedAccessPolicy,
                       previousWorkspaceId, hadRuntimeWorkspace, generation);
    return true;
  }

  Q_INVOKABLE bool createChannel(const QString &name, bool isPrivate) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto channelName = name.trimmed();
    if (channelName.isEmpty()) {
      setSyncStatus(QStringLiteral("room name required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            channelName, kMaxChannelNameBytes, QStringLiteral("room name"),
            QStringLiteral("128 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("creating room..."));
    runChannelCreate(channelName, isPrivate, generation);
    return true;
  }

  Q_INVOKABLE bool updateChannelDetails(const QString &channelId,
                                        const QString &name,
                                        const QString &topic) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedChannelId = channelId.trimmed();
    const auto channelName = name.trimmed();
    const auto channelTopic = topic.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }
    if (channelName.isEmpty()) {
      setSyncStatus(QStringLiteral("room name required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedChannelId, kMaxChannelIdBytes, QStringLiteral("room ID"),
            QStringLiteral("128 bytes"), &metadataError) ||
        !validateMetadataTextForWrite(
            channelName, kMaxChannelNameBytes, QStringLiteral("room name"),
            QStringLiteral("128 bytes"), &metadataError) ||
        !validateMetadataTextForWrite(
            channelTopic, kMaxChannelTopicBytes, QStringLiteral("room topic"),
            QStringLiteral("512 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_updateChannelDetailsJson == nullptr) {
      setSyncStatus(QStringLiteral("room details unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("saving room details..."));
    runChannelDetailsUpdate(normalizedChannelId, channelName, channelTopic,
                            generation);
    return true;
  }

  Q_INVOKABLE bool updateChannelArchive(const QString &channelId,
                                        bool archived) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedChannelId = channelId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedChannelId, kMaxChannelIdBytes, QStringLiteral("room ID"),
            QStringLiteral("128 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_updateChannelArchiveJson == nullptr) {
      setSyncStatus(QStringLiteral("room archive unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(archived ? QStringLiteral("archiving room...")
                           : QStringLiteral("restoring room..."));
    runChannelArchiveUpdate(normalizedChannelId, archived, generation);
    return true;
  }

  Q_INVOKABLE bool createDirectMessage(const QString &deviceId,
                                       const QString &displayName) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_syncInFlight) {
      setSyncStatus(QStringLiteral("another workspace update is still running"));
      return false;
    }

    const auto normalizedDeviceId = deviceId.trimmed();
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("person required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("person support code"), QStringLiteral("512 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_createDirectMessageChannelJson == nullptr) {
      setSyncStatus(QStringLiteral("direct messages unavailable"));
      return false;
    }

    const auto channelName =
        directMessageChannelName(displayName, normalizedDeviceId);
    if (!validateMetadataTextForWrite(
            channelName, kMaxChannelNameBytes, QStringLiteral("room name"),
            QStringLiteral("128 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncInFlight(true);
    setSyncStatus(QStringLiteral("starting direct message..."));
    runDirectMessageCreate(channelName, normalizedDeviceId, generation);
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
      setSyncStatus(QStringLiteral("room list paging unavailable"));
      return false;
    }

    const auto channels =
        m_workspaceSnapshot.value(QStringLiteral("channels")).toList();
    const auto channelCount =
        m_workspaceSnapshot.value(QStringLiteral("channelCount")).toULongLong();
    const auto startIndex = static_cast<std::size_t>(channels.size());
    if (channelCount > 0 &&
        static_cast<qulonglong>(startIndex) >= channelCount) {
      setSyncStatus(QStringLiteral("all rooms loaded"));
      return false;
    }

    const auto generation = ++m_channelPageGeneration;
    setChannelPageInFlight(true);
    setSyncStatus(QStringLiteral("loading rooms..."));
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
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("room ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_channelPageInFlight) {
      return false;
    }
    if (m_listWorkspaceChannelPageContainingJson == nullptr) {
      setSyncStatus(QStringLiteral("room lookup unavailable"));
      return false;
    }

    const auto generation = ++m_channelPageGeneration;
    setChannelPageInFlight(true);
    setSyncStatus(QStringLiteral("loading room..."));
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
      setSyncStatus(QStringLiteral("name required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedDisplayName, kMaxDeviceDisplayNameBytes,
            QStringLiteral("name"), QStringLiteral("128 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_updateDeviceProfileJson == nullptr) {
      setSyncStatus(QStringLiteral("profile update unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("saving name..."));
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
                                      QStringLiteral("room ID"),
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
                                      QStringLiteral("room ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(normalizedReplyTo, kMaxMessageIdBytes,
                                      QStringLiteral("message"),
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
                                      QStringLiteral("room ID"),
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
                                      QStringLiteral("room ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedReplyTo, kMaxMessageIdBytes, QStringLiteral("message"),
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
      setSyncStatus(QStringLiteral("access file type required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedProtocol, kMaxDeviceKeyPackageProtocolBytes,
            QStringLiteral("access file type"), QStringLiteral("128 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (normalizedFilePath.isEmpty()) {
      setSyncStatus(QStringLiteral("access file required"));
      return false;
    }
    if (!validateMetadataTextForWrite(normalizedFilePath, kMaxFfiPathBytes,
                                      QStringLiteral("access file path"),
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
      setSyncStatus(QStringLiteral("access file sharing unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("sharing access file..."));
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
      setSyncStatus(QStringLiteral("address name required"));
      return false;
    }
    if (normalizedEndpoint.isEmpty()) {
      setSyncStatus(QStringLiteral("sharing address required"));
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
      setSyncStatus(QStringLiteral("address sharing unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("sharing address..."));
    runPeerEndpointPublish(normalizedEndpointId, normalizedEndpoint,
                           normalizedTransport, isBackupPeer, false, 0,
                           QStringLiteral("address shared"), generation,
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
        QStringLiteral("access details unavailable"),
        QStringLiteral("access details shared"));
  }

  Q_INVOKABLE bool createOpenMlsWorkspaceGroup() {
    return callWorkspaceOpenMlsAction(
        m_createOpenMlsWorkspaceGroupJson,
        QStringLiteral("workspace access unavailable"),
        QStringLiteral("workspace access ready"));
  }

  Q_INVOKABLE bool addOpenMlsWorkspaceGroupMember(const QString &keyPackageId) {
    return callWorkspaceOpenMlsValueAction(
        m_addOpenMlsWorkspaceGroupMemberJson, keyPackageId, false,
        QStringLiteral("access details required"),
        QStringLiteral("workspace access unavailable"),
        QStringLiteral("workspace access granted"));
  }

  Q_INVOKABLE bool
  joinOpenMlsWorkspaceGroup(const QString &sourceEventId = QString()) {
    return callWorkspaceOpenMlsValueAction(
        m_joinOpenMlsWorkspaceGroupJson, sourceEventId, true, QString(),
        QStringLiteral("workspace access unavailable"),
        QStringLiteral("workspace access accepted"));
  }

  Q_INVOKABLE bool updateOpenMlsWorkspaceGroup() {
    return callWorkspaceOpenMlsAction(
        m_updateOpenMlsWorkspaceGroupJson,
        QStringLiteral("workspace access refresh unavailable"),
        QStringLiteral("workspace access refreshed"));
  }

  Q_INVOKABLE bool updateWorkspaceOpenMlsGroups() {
    return callWorkspaceOpenMlsAction(
        m_updateWorkspaceOpenMlsGroupsJson,
        QStringLiteral("access refresh unavailable"),
        QStringLiteral("access refreshed"));
  }

  Q_INVOKABLE bool
  applyOpenMlsWorkspaceGroupCommits(const QString &sourceEventId = QString()) {
    return callWorkspaceOpenMlsValueAction(
        m_applyOpenMlsWorkspaceGroupCommitsJson, sourceEventId, true, QString(),
        QStringLiteral("workspace access changes unavailable"),
        QStringLiteral("workspace access changes applied"));
  }

  Q_INVOKABLE bool createOpenMlsChannelGroup(const QString &channelId) {
    return callChannelOpenMlsAction(
        m_createOpenMlsChannelGroupJson, channelId,
        QStringLiteral("room access unavailable"),
        QStringLiteral("room access ready"));
  }

  Q_INVOKABLE bool addOpenMlsChannelGroupMember(const QString &channelId,
                                                const QString &keyPackageId) {
    return callChannelOpenMlsValueAction(
        m_addOpenMlsChannelGroupMemberJson, channelId, keyPackageId, false,
        QStringLiteral("access details required"),
        QStringLiteral("room access unavailable"),
        QStringLiteral("room access granted"));
  }

  Q_INVOKABLE bool
  joinOpenMlsChannelGroup(const QString &channelId,
                          const QString &sourceEventId = QString()) {
    return callChannelOpenMlsValueAction(
        m_joinOpenMlsChannelGroupJson, channelId, sourceEventId, true,
        QString(), QStringLiteral("room access unavailable"),
        QStringLiteral("room access accepted"));
  }

  Q_INVOKABLE bool updateOpenMlsChannelGroup(const QString &channelId) {
    return callChannelOpenMlsAction(
        m_updateOpenMlsChannelGroupJson, channelId,
        QStringLiteral("room access refresh unavailable"),
        QStringLiteral("room access refreshed"));
  }

  Q_INVOKABLE bool
  applyOpenMlsChannelGroupCommits(const QString &channelId,
                                  const QString &sourceEventId = QString()) {
    return callChannelOpenMlsValueAction(
        m_applyOpenMlsChannelGroupCommitsJson, channelId, sourceEventId, true,
        QString(), QStringLiteral("room access changes unavailable"),
        QStringLiteral("room access changes applied"));
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
      setSyncStatus(QStringLiteral("message required"));
      return false;
    }
    if (normalizedAttachmentSelector.isEmpty()) {
      setSyncStatus(QStringLiteral("attachment required"));
      return false;
    }
    if (normalizedOutputPath.isEmpty()) {
      setSyncStatus(QStringLiteral("save location required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedMessageId, kMaxMessageIdBytes,
                                      QStringLiteral("message"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedAttachmentSelector, kMaxAttachmentSelectorBytes,
            QStringLiteral("attachment"), QStringLiteral("256 bytes"),
            &metadataError) ||
        !validateMetadataTextForWrite(normalizedOutputPath, kMaxFfiPathBytes,
                                      QStringLiteral("save location"),
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
      setSyncStatus(QStringLiteral("file cleanup unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("cleaning up files..."));
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
      setSyncStatus(QStringLiteral("message required"));
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
                                      QStringLiteral("message"),
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
      setSyncStatus(QStringLiteral("message required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedMessageId, kMaxMessageIdBytes,
                                      QStringLiteral("message"),
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
      setSyncStatus(QStringLiteral("message required"));
      return false;
    }
    if (normalizedReaction.isEmpty()) {
      setSyncStatus(QStringLiteral("reaction required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedMessageId, kMaxMessageIdBytes,
                                      QStringLiteral("message"),
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
      setSyncStatus(QStringLiteral("message required"));
      return false;
    }
    if (normalizedReaction.isEmpty()) {
      setSyncStatus(QStringLiteral("reaction required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedMessageId, kMaxMessageIdBytes,
                                      QStringLiteral("message"),
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
                                      QStringLiteral("room ID"),
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
      setSyncStatus(QStringLiteral("support code required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("support code"), QStringLiteral("512 bytes"),
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
    setSyncStatus(QStringLiteral("creating invite..."));
    runMemberInvite(normalizedDeviceId, normalizedRole, generation);
    return true;
  }

  Q_INVOKABLE bool recordWorkspaceJoinRequest(const QString &requestId,
                                              const QString &deviceId,
                                              const QString &displayName,
                                              const QString &note,
                                              const QString &sourceType,
                                              const QString &sourceInviteId,
                                              const QString &sourceDisplayName,
                                              const QString &sourceApprovalPolicy,
                                              const QString &responsePeerEndpoint = QString()) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_recordWorkspaceJoinRequestJson == nullptr) {
      setSyncStatus(QStringLiteral("access request tracking unavailable"));
      return false;
    }

    auto normalizedRequestId = requestId.trimmed();
    if (normalizedRequestId.isEmpty()) {
      normalizedRequestId = QStringLiteral("req_") +
                            QUuid::createUuid().toString(QUuid::WithoutBraces);
    }
    const auto normalizedDeviceId = deviceId.trimmed();
    const auto normalizedDisplayName = displayName.trimmed();
    const auto normalizedNote = note.trimmed();
    const auto normalizedSourceType = sourceType.trimmed();
    const auto normalizedSourceInviteId = sourceInviteId.trimmed();
    const auto normalizedSourceDisplayName = sourceDisplayName.trimmed();
    const auto normalizedSourceApprovalPolicy = sourceApprovalPolicy.trimmed();
    const auto normalizedResponsePeerEndpoint = responsePeerEndpoint.trimmed();
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("support code required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedRequestId, kMaxJoinRequestIdBytes,
            QStringLiteral("access request ID"), QStringLiteral("128 bytes"),
            &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("support code"), QStringLiteral("512 bytes"),
            &metadataError) ||
        (!normalizedDisplayName.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedDisplayName, kMaxDeviceDisplayNameBytes,
             QStringLiteral("display name"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        (!normalizedNote.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedNote, kMaxJoinRequestNoteBytes,
             QStringLiteral("access request note"), QStringLiteral("512 bytes"),
             &metadataError)) ||
        (!normalizedSourceType.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedSourceType, kMaxWorkspaceAccessPolicyBytes,
             QStringLiteral("request source"), QStringLiteral("32 bytes"),
             &metadataError)) ||
        (!normalizedSourceInviteId.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedSourceInviteId, kMaxInviteIdBytes,
             QStringLiteral("source invite ID"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        (!normalizedSourceDisplayName.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedSourceDisplayName, kMaxDeviceDisplayNameBytes,
             QStringLiteral("source display name"), QStringLiteral("128 bytes"),
             &metadataError)) ||
        (!normalizedSourceApprovalPolicy.isEmpty() &&
         !validateMetadataTextForWrite(
             normalizedSourceApprovalPolicy, kMaxInviteApprovalPolicyBytes,
             QStringLiteral("source approval policy"), QStringLiteral("32 bytes"),
             &metadataError)) ||
        (!normalizedResponsePeerEndpoint.isEmpty() &&
         !validatePeerEndpointForUse(normalizedResponsePeerEndpoint,
                                     &metadataError))) {
      setSyncStatus(metadataError);
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("recording access request..."));
    runWorkspaceJoinRequestRecord(normalizedRequestId, normalizedDeviceId,
                                  normalizedDisplayName, normalizedNote,
                                  normalizedSourceType, normalizedSourceInviteId,
                                  normalizedSourceDisplayName,
                                  normalizedSourceApprovalPolicy,
                                  normalizedResponsePeerEndpoint,
                                  generation);
    return true;
  }

  Q_INVOKABLE bool resolveWorkspaceJoinRequest(
      const QString &requestId, const QString &resolution,
      const QString &responseDeliveryPeerEndpoint = QString()) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_resolveWorkspaceJoinRequestJson == nullptr) {
      setSyncStatus(QStringLiteral("access request tracking unavailable"));
      return false;
    }

    const auto normalizedRequestId = requestId.trimmed();
    const auto normalizedResolution = resolution.trimmed();
    const auto normalizedResponseDeliveryPeerEndpoint =
        responseDeliveryPeerEndpoint.trimmed();
    if (normalizedRequestId.isEmpty()) {
      setSyncStatus(QStringLiteral("access request missing"));
      return false;
    }
    if (normalizedResolution != QStringLiteral("approved") &&
        normalizedResolution != QStringLiteral("declined") &&
        normalizedResolution != QStringLiteral("revoked")) {
      setSyncStatus(QStringLiteral("access request action unavailable"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedRequestId, kMaxJoinRequestIdBytes,
            QStringLiteral("access request ID"), QStringLiteral("128 bytes"),
            &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedResolution, kMaxWorkspaceRoleBytes,
            QStringLiteral("access request action"), QStringLiteral("16 bytes"),
            &metadataError) ||
        (!normalizedResponseDeliveryPeerEndpoint.isEmpty() &&
         !validatePeerEndpointForUse(normalizedResponseDeliveryPeerEndpoint,
                                     &metadataError))) {
      setSyncStatus(metadataError);
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("updating access request..."));
    runWorkspaceJoinRequestResolve(normalizedRequestId, normalizedResolution,
                                   normalizedResponseDeliveryPeerEndpoint,
                                   generation);
    return true;
  }

  Q_INVOKABLE bool refreshJoinRequestInbox() {
    return queueJoinRequestInboxRefresh(true);
  }

  Q_INVOKABLE bool resolveWorkspaceInvite(const QString &inviteId,
                                          const QString &resolution) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_resolveWorkspaceInviteJson == nullptr) {
      setSyncStatus(QStringLiteral("invite tracking unavailable"));
      return false;
    }

    const auto normalizedInviteId = inviteId.trimmed();
    const auto normalizedResolution = resolution.trimmed();
    if (normalizedInviteId.isEmpty()) {
      setSyncStatus(QStringLiteral("invite missing"));
      return false;
    }
    if (normalizedResolution != QStringLiteral("revoked")) {
      setSyncStatus(QStringLiteral("invite action unavailable"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedInviteId, kMaxInviteIdBytes, QStringLiteral("invite ID"),
            QStringLiteral("128 bytes"), &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedResolution, kMaxWorkspaceRoleBytes,
            QStringLiteral("invite action"), QStringLiteral("16 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("updating invite..."));
    runWorkspaceInviteResolve(normalizedInviteId, normalizedResolution,
                              generation);
    return true;
  }

  Q_INVOKABLE bool updateMemberRole(const QString &deviceId,
                                    const QString &role) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedDeviceId = deviceId.trimmed();
    const auto normalizedRole =
        role.trimmed().isEmpty() ? QStringLiteral("member") : role.trimmed();
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("person required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("person support code"), QStringLiteral("512 bytes"),
            &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedRole, kMaxWorkspaceRoleBytes,
            QStringLiteral("workspace role"), QStringLiteral("16 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_updateMemberRoleJson == nullptr) {
      setSyncStatus(QStringLiteral("role changes unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("updating role..."));
    runMemberRoleUpdate(normalizedDeviceId, normalizedRole, generation);
    return true;
  }

  Q_INVOKABLE bool updateWorkspaceAccessPolicy(const QString &accessPolicy) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedAccessPolicy =
        normalizedWorkspaceAccessPolicy(accessPolicy);
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedAccessPolicy, kMaxWorkspaceAccessPolicyBytes,
            QStringLiteral("workspace access policy"), QStringLiteral("32 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_updateWorkspaceAccessPolicyJson == nullptr) {
      setSyncStatus(QStringLiteral("workspace access settings unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("updating workspace access..."));
    runWorkspaceAccessPolicyUpdate(normalizedAccessPolicy, generation);
    return true;
  }

  Q_INVOKABLE bool removeMember(const QString &deviceId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }

    const auto normalizedDeviceId = deviceId.trimmed();
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("person required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("person support code"), QStringLiteral("512 bytes"),
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
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("support code required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("room ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("support code"), QStringLiteral("512 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_addChannelMemberJson == nullptr) {
      setSyncStatus(QStringLiteral("room access changes unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("granting room access..."));
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
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }
    if (normalizedDeviceId.isEmpty()) {
      setSyncStatus(QStringLiteral("support code required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("room ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedDeviceId, kMaxDeviceIdReferenceBytes,
            QStringLiteral("support code"), QStringLiteral("512 bytes"),
            &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_removeChannelMemberWithOpenMlsJson == nullptr &&
        m_removeChannelMemberWithKeyRotationJson == nullptr) {
      setSyncStatus(QStringLiteral("room access removal unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("removing room access..."));
    runChannelMemberRemove(normalizedChannelId, normalizedDeviceId, generation);
    return true;
  }

  Q_INVOKABLE bool exportWorkspaceKey() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    if (m_exportWorkspaceKeyJson == nullptr) {
      setSyncStatus(QStringLiteral("workspace access export unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("creating workspace access file..."));
    runWorkspaceJsonExport(m_exportWorkspaceKeyJson,
                           QStringLiteral("workspace access file ready"));
    return true;
  }

  Q_INVOKABLE bool exportTrustSnapshot() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    if (m_exportTrustSnapshotJson == nullptr) {
      setSyncStatus(QStringLiteral("access record export unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("exporting access record..."));
    runWorkspaceJsonExport(m_exportTrustSnapshotJson,
                           QStringLiteral("access record exported"));
    return true;
  }

  Q_INVOKABLE bool rotateWorkspaceManualKeys() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    if (m_rotateWorkspaceForSuspectedCompromiseJson == nullptr) {
      setSyncStatus(QStringLiteral("access refresh unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("refreshing workspace access..."));
    runWorkspaceCompromiseRotation(generation);
    return true;
  }

  Q_INVOKABLE bool detectCompromise() {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
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
      setSyncStatus(QStringLiteral("open a workspace to import access"));
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    const auto normalizedKeyJson = keyJson.trimmed();
    if (normalizedKeyJson.isEmpty()) {
      setSyncStatus(QStringLiteral("workspace access file required"));
      return false;
    }
    QString jsonError;
    if (!validateJsonTextForImport(normalizedKeyJson, kMaxKeyTransferJsonBytes,
                                   QStringLiteral("workspace access file"),
                                   QStringLiteral("256 KB"), &jsonError)) {
      setSyncStatus(jsonError);
      return false;
    }
    if (m_importWorkspaceKeyJson == nullptr) {
      setSyncStatus(QStringLiteral("workspace access import unavailable"));
      return false;
    }

    const auto hadRuntimeWorkspace = hasRuntimeWorkspace();
    const auto previousWorkspaceId = m_workspaceId;
    const auto generation = ++m_runtimeWriteGeneration;
    setLastRecoveryImportedChannelCount(0);
    setSyncStatus(QStringLiteral("importing workspace access..."));
    runWorkspaceKeyImport(m_importWorkspaceKeyJson, normalizedKeyJson,
                          previousWorkspaceId, hadRuntimeWorkspace, generation,
                          QStringLiteral("workspace access imported"));
    return true;
  }

  Q_INVOKABLE bool exportChannelKey(const QString &channelId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    const auto normalizedChannelId = channelId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("room ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_exportChannelKeyJson == nullptr) {
      setSyncStatus(QStringLiteral("room access export unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("creating room access file..."));
    runChannelKeyExport(normalizedChannelId,
                        QStringLiteral("room access file ready"));
    return true;
  }

  Q_INVOKABLE bool rotateChannelKey(const QString &channelId) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    const auto normalizedChannelId = channelId.trimmed();
    if (normalizedChannelId.isEmpty()) {
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("room ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_rotateChannelKeyJson == nullptr) {
      setSyncStatus(QStringLiteral("room access refresh unavailable"));
      return false;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("refreshing room access..."));
    runChannelKeyRotation(normalizedChannelId, generation);
    return true;
  }

  Q_INVOKABLE bool importChannelKey(const QString &keyJson) {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_rawEventStoreMode) {
      setSyncStatus(QStringLiteral("open a workspace to import room access"));
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    const auto normalizedKeyJson = keyJson.trimmed();
    if (normalizedKeyJson.isEmpty()) {
      setSyncStatus(QStringLiteral("room access file required"));
      return false;
    }
    QString jsonError;
    if (!validateJsonTextForImport(normalizedKeyJson, kMaxKeyTransferJsonBytes,
                                   QStringLiteral("room access file"),
                                   QStringLiteral("256 KB"), &jsonError)) {
      setSyncStatus(jsonError);
      return false;
    }
    if (m_importChannelKeyJson == nullptr) {
      setSyncStatus(QStringLiteral("room access import unavailable"));
      return false;
    }

    const auto hadRuntimeWorkspace = hasRuntimeWorkspace();
    const auto previousWorkspaceId = m_workspaceId;
    const auto generation = ++m_runtimeWriteGeneration;
    setSyncStatus(QStringLiteral("importing room access..."));
    runChannelKeyImport(normalizedKeyJson, previousWorkspaceId,
                        hadRuntimeWorkspace, generation);
    return true;
  }

  Q_INVOKABLE bool exportRecoveryBundle(const QString &passphrase) {
    if (!ensureRuntimeWorkspace()) {
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
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
      setSyncStatus(QStringLiteral("recovery kit export unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("creating recovery kit..."));
    runRecoveryBundleExport(passphrase,
                            QStringLiteral("recovery kit ready"));
    return true;
  }

  Q_INVOKABLE bool importRecoveryBundle(const QString &bundleJson,
                                        const QString &passphrase) {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_rawEventStoreMode) {
      setSyncStatus(QStringLiteral("open a workspace to restore access"));
      return false;
    }
    if (m_keyTransferInFlight) {
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    const auto normalizedBundleJson = bundleJson.trimmed();
    if (normalizedBundleJson.isEmpty()) {
      setSyncStatus(QStringLiteral("recovery kit required"));
      return false;
    }
    QString jsonError;
    if (!validateJsonTextForImport(normalizedBundleJson,
                                   kMaxRecoveryBundleJsonBytes,
                                   QStringLiteral("recovery kit"),
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
      setSyncStatus(QStringLiteral("recovery kit import unavailable"));
      return false;
    }

    const auto hadRuntimeWorkspace = hasRuntimeWorkspace();
    const auto previousWorkspaceId = m_workspaceId;
    const auto generation = ++m_runtimeWriteGeneration;
    setLastRecoveryImportedChannelCount(0);
    setSyncStatus(QStringLiteral("importing recovery kit..."));
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
      setSyncStatus(QStringLiteral("access handoff already running"));
      return false;
    }
    if (m_reindexWorkspaceSearchJson == nullptr) {
      setSyncStatus(QStringLiteral("search refresh unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("refreshing search..."));
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
      setSyncStatus(QStringLiteral("room search unavailable"));
      return false;
    }

    runWorkspaceChannelSearch(normalizedQuery, generation);
    return true;
  }

  void startBackgroundReachability() {
    if (!m_ffiReady || m_rawEventStoreMode || m_peerHostingInFlight ||
        peerHosting() || m_backgroundReachabilityStoppedByUser ||
        !backgroundReachabilityEnabled()) {
      return;
    }
    m_backgroundReachabilityRetryScheduled = false;
    if (m_startIrohPeerJson != nullptr) {
      m_backgroundReachabilityFallbackPending = true;
      setSyncStatus(QStringLiteral("connecting to peers..."));
      runIrohPeerStart();
      return;
    }
    if (developmentLoopbackFallbackEnabled() &&
        m_startDirectPeerJson != nullptr) {
      m_backgroundReachabilityFallbackPending = true;
      setSyncStatus(QStringLiteral("starting local peer access..."));
      runDirectPeerStart(QStringLiteral("127.0.0.1:0"));
      return;
    }
    m_backgroundReachabilityFallbackPending = false;
    setSyncStatus(QStringLiteral("secure peer transport unavailable"));
  }

  void scheduleBackgroundReachabilityRetry(const QString &error) {
    m_backgroundReachabilityFallbackPending = false;
    if (!backgroundReachabilityEnabled() ||
        m_backgroundReachabilityStoppedByUser ||
        m_backgroundReachabilityRetryScheduled || peerHosting()) {
      if (!error.trimmed().isEmpty()) {
        setSyncStatus(error);
      }
      return;
    }

    const auto exponent = std::min(m_backgroundReachabilityRetryAttempt, 6);
    const auto delayMs = std::min(60'000, 1'000 * (1 << exponent));
    ++m_backgroundReachabilityRetryAttempt;
    m_backgroundReachabilityRetryScheduled = true;
    setSyncStatus(QStringLiteral("peer connection unavailable; retrying in %1s")
                      .arg((delayMs + 999) / 1000));
    const QPointer<ChaftController> guard(this);
    QTimer::singleShot(delayMs, this, [guard]() {
      if (guard.isNull()) {
        return;
      }
      guard->m_backgroundReachabilityRetryScheduled = false;
      guard->startBackgroundReachability();
    });
  }

  Q_INVOKABLE bool startLocalPeer(const QString &listenEndpoint) {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_peerHostingInFlight) {
      setSyncStatus(QStringLiteral("address sharing already updating"));
      return false;
    }
    if (peerHosting()) {
      setSyncStatus(
          QStringLiteral("sharing address %1").arg(m_hostedPeerEndpoint));
      queueJoinRequestInboxRefresh(false);
      return true;
    }
    if (m_startDirectPeerJson == nullptr) {
      setSyncStatus(QStringLiteral("address sharing unavailable"));
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
    m_backgroundReachabilityStoppedByUser = false;
    m_backgroundReachabilityFallbackPending = false;
    setSyncStatus(QStringLiteral("creating sharing address..."));
    runDirectPeerStart(listen);
    return true;
  }

  Q_INVOKABLE bool startLocalIrohPeer() {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_peerHostingInFlight) {
      setSyncStatus(QStringLiteral("address sharing already updating"));
      return false;
    }
    if (peerHosting()) {
      setSyncStatus(
          QStringLiteral("sharing address %1").arg(m_hostedPeerEndpoint));
      queueJoinRequestInboxRefresh(false);
      return true;
    }
    if (m_startIrohPeerJson == nullptr) {
      setSyncStatus(QStringLiteral("relay address unavailable"));
      return false;
    }

    m_backgroundReachabilityStoppedByUser = false;
    m_backgroundReachabilityFallbackPending = false;
    setSyncStatus(QStringLiteral("creating relay address..."));
    runIrohPeerStart();
    return true;
  }

  Q_INVOKABLE bool stopLocalPeer() {
    m_backgroundReachabilityStoppedByUser = true;
    m_backgroundReachabilityFallbackPending = false;
    m_backgroundReachabilityRetryScheduled = false;
    if (m_hostedPeerId.isEmpty()) {
      return true;
    }
    if (m_peerHostingInFlight) {
      setSyncStatus(QStringLiteral("address sharing already updating"));
      return false;
    }
    if (m_stopDirectPeerJson == nullptr) {
      setSyncStatus(QStringLiteral("address sharing controls unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("stopping address sharing..."));
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
                              QStringLiteral("sharing address refreshed"));
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
      setSyncStatus(QStringLiteral("teammate address required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointForUse(endpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    setDefaultPeerEndpoint(endpoint);
    setSyncStatus(QStringLiteral("sharing updates..."));
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
      setSyncStatus(QStringLiteral("backup devices cooling down"));
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
      setSyncStatus(QStringLiteral("message support sharing unavailable"));
      return false;
    }

    const auto normalizedEventId = eventId.trimmed();
    if (normalizedEventId.isEmpty()) {
      setSyncStatus(QStringLiteral("message record required"));
      return false;
    }
    const auto endpoint = peerEndpoint.trimmed();
    if (endpoint.isEmpty()) {
      setSyncStatus(QStringLiteral("teammate address required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(
            normalizedEventId, kMaxEventIdBytes, QStringLiteral("message record"),
            QStringLiteral("68 bytes"), &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (!isCanonicalEventId(normalizedEventId)) {
      setSyncStatus(QStringLiteral("message record is invalid"));
      return false;
    }
    if (!validatePeerEndpointForUse(endpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    setDefaultPeerEndpoint(endpoint);
    setSyncStatus(QStringLiteral("sharing message support info..."));
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
      setSyncStatus(QStringLiteral("teammate address required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointForUse(endpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    setDefaultPeerEndpoint(endpoint);
    setSyncStatus(QStringLiteral("fetching updates..."));
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
      setSyncStatus(QStringLiteral("teammate address required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointForUse(endpoint, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    setDefaultPeerEndpoint(endpoint);
    setSyncStatus(QStringLiteral("sharing and fetching updates..."));
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
      setSyncStatus(QStringLiteral("history fix unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("fixing history..."));
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
              &workspaceId, false, QStringLiteral("workspace required"))) {
        return false;
      }
      if (m_storeSnapshotWindowJson == nullptr) {
        setSyncStatus(QStringLiteral("message history paging unavailable"));
        return false;
      }
    } else {
      if (!ensureRuntimeWorkspace()) {
        return false;
      }
      if (m_runtimeSnapshotWindowJson == nullptr) {
        setSyncStatus(QStringLiteral("message history paging unavailable"));
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
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("room ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_runtimeChannelSnapshotLatestJson == nullptr) {
      setSyncStatus(QStringLiteral("room history unavailable"));
      return false;
    }

    setSyncStatus(QStringLiteral("loading room history..."));
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
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("room ID"),
                                      QStringLiteral("128 bytes"),
                                      &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }
    if (m_runtimeChannelSnapshotWindowJson == nullptr) {
      setSyncStatus(QStringLiteral("room history paging unavailable"));
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

    setSyncStatus(QStringLiteral("loading older room history..."));
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
      setSyncStatus(QStringLiteral("file retry unavailable"));
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
      setSyncStatus(QStringLiteral("teammate address required"));
      return false;
    }
    QString metadataError;
    if (!validatePeerEndpointListForUse(peerEndpoints, &metadataError)) {
      setSyncStatus(metadataError);
      return false;
    }

    setSyncStatus(QStringLiteral("retrying files..."));
    runBlobTransferRetry(peerEndpoints);
    return true;
  }

signals:
  void workspaceSnapshotChanged();
  void workspaceSummariesChanged();
  void selectedWorkspaceChanged();
  void syncStatusChanged();
  void lastRecoveryImportedChannelCountChanged();
  void defaultPeerEndpointChanged();
  void backupPeerEndpointsChanged();
  void backupPeerStatusesChanged();
  void publishQueueChanged();
  void workspaceStorageHealthChanged();
  void autoBackupEnabledChanged();
  void themeIdChanged();
  void inspectorPinnedChanged();
  void reducedMotionEnabledChanged();
  void notificationSettingsChanged();
  void externalLinkSettingsChanged();
  void mutedChannelsChanged();
  void composerDraftsChanged();
  void pendingJoinRequestsChanged();
  void windowGeometryChanged();
  void lastCreatedChannelChanged();
  void themeModeChanged();
  void darkThemeIdChanged();
  void lightThemeIdChanged();
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
  void joinRequestSubmitInFlightChanged();
  void joinRequestInboxInFlightChanged();
  void accessEnvelopePullInFlightChanged();
  void joinRequestDirectSubmitFinished(bool success, const QString &message);

private:
  static const char *nullableUtf8(const QByteArray &value) {
    return value.isEmpty() ? nullptr : value.constData();
  }

  bool ensureNotificationTrayIcon() {
    if (!QSystemTrayIcon::isSystemTrayAvailable()) {
      return false;
    }
    if (m_notificationTrayIcon == nullptr) {
      auto trayIcon = std::make_unique<QSystemTrayIcon>(desktopNotificationIcon(),
                                                        this);
      trayIcon->setToolTip(QStringLiteral("Chaft"));
      trayIcon->show();
      m_notificationTrayIcon = std::move(trayIcon);
    }
    return m_notificationTrayIcon != nullptr;
  }

  bool storeRuntimeUnlockPassphrase(const QString &passphrase) {
    if (m_setIdentityPassphrase == nullptr) {
      setRuntimeUnlockRequired(true);
      setSyncStatus(QStringLiteral("saved unlock unavailable"));
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
      setSyncStatus(QStringLiteral("workspace unlock failed"));
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
      m_createDirectMessageChannelJson =
          reinterpret_cast<RuntimeCreateDirectMessageChannelResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_create_direct_message_channel_result_json"));
      m_updateChannelDetailsJson =
          reinterpret_cast<RuntimeUpdateChannelDetailsResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_update_channel_details_result_json"));
      m_updateChannelArchiveJson =
          reinterpret_cast<RuntimeUpdateChannelArchiveResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_update_channel_archive_result_json"));
      m_updateDeviceProfileJson =
          reinterpret_cast<RuntimeUpdateDeviceProfileResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_update_device_profile_result_json"));
      m_updateLocalPersonProfileJson =
          reinterpret_cast<RuntimeUpdateLocalPersonProfileResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_update_local_person_profile_result_json"));
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
      m_createWorkspaceWithAccessPolicyJson =
          reinterpret_cast<RuntimeCreateWorkspaceWithAccessPolicyResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_create_workspace_with_access_policy_result_json"));
      m_inviteMemberJson = reinterpret_cast<RuntimeInviteMemberResultJsonFn>(
          m_library.resolve("chaft_runtime_invite_member_result_json"));
      m_createWorkspaceInviteJson =
          reinterpret_cast<RuntimeCreateWorkspaceInviteResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_create_workspace_invite_result_json"));
      m_prepareWorkspaceInviteClaimJson =
          reinterpret_cast<RuntimePrepareWorkspaceInviteClaimResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_prepare_workspace_invite_claim_result_json"));
      m_claimWorkspaceInviteJson =
          reinterpret_cast<RuntimeWorkspaceInviteEnvelopeResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_claim_workspace_invite_result_json"));
      m_importWorkspaceInviteResponseJson =
          reinterpret_cast<RuntimeWorkspaceInviteEnvelopeResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_import_workspace_invite_response_result_json"));
      m_recordWorkspaceInviteJson =
          reinterpret_cast<RuntimeRecordWorkspaceInviteResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_record_workspace_invite_result_json"));
      m_resolveWorkspaceInviteJson =
          reinterpret_cast<RuntimeResolveWorkspaceInviteResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_resolve_workspace_invite_result_json"));
      m_recordWorkspaceJoinRequestJson =
          reinterpret_cast<RuntimeRecordWorkspaceJoinRequestResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_record_workspace_join_request_result_json"));
      m_recordWorkspaceJoinRequestWithResponseRouteJson = reinterpret_cast<
          RuntimeRecordWorkspaceJoinRequestWithResponseRouteResultJsonFn>(
          m_library.resolve(
              "chaft_runtime_record_workspace_join_request_with_response_route_"
              "result_json"));
      m_resolveWorkspaceJoinRequestJson =
          reinterpret_cast<RuntimeResolveWorkspaceJoinRequestResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_resolve_workspace_join_request_result_json"));
      m_updateMemberRoleJson =
          reinterpret_cast<RuntimeUpdateMemberRoleResultJsonFn>(
              m_library.resolve("chaft_runtime_update_member_role_result_json"));
      m_updateWorkspaceAccessPolicyJson =
          reinterpret_cast<RuntimeUpdateWorkspaceAccessPolicyResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_update_workspace_access_policy_result_json"));
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
      m_submitJoinRequestDirectJson =
          reinterpret_cast<RuntimeSubmitJoinRequestDirectResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_submit_join_request_direct_result_json"));
      m_pullJoinRequestsDirectJson =
          reinterpret_cast<RuntimePullJoinAccessDirectResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_pull_join_requests_direct_result_json"));
      m_pullJoinResponsesDirectJson =
          reinterpret_cast<RuntimePullJoinAccessDirectResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_pull_join_responses_direct_result_json"));
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
      m_listJoinRequestInboxJson =
          reinterpret_cast<RuntimeListJoinRequestInboxResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_list_join_request_inbox_result_json"));
      m_ackJoinRequestInboxEntryJson =
          reinterpret_cast<RuntimeAckJoinRequestInboxEntryResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_ack_join_request_inbox_entry_result_json"));
      m_queueJoinRequestOutboxJson =
          reinterpret_cast<RuntimeQueueJoinRequestOutboxResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_queue_join_request_outbox_result_json"));
      m_listJoinRequestOutboxJson =
          reinterpret_cast<RuntimeListJoinRequestOutboxResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_list_join_request_outbox_result_json"));
      m_listDueJoinRequestOutboxJson =
          reinterpret_cast<RuntimeListJoinRequestOutboxResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_list_due_join_request_outbox_result_json"));
      m_submitJoinRequestOutboxEntryDirectJson =
          reinterpret_cast<RuntimeSubmitJoinRequestOutboxEntryDirectResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_submit_join_request_outbox_entry_direct_result_"
                  "json"));
      m_listJoinResponseInboxJson =
          reinterpret_cast<RuntimeListJoinResponseInboxResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_list_join_response_inbox_result_json"));
      m_ackJoinResponseInboxEntryJson =
          reinterpret_cast<RuntimeAckJoinResponseInboxEntryResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_ack_join_response_inbox_entry_result_json"));
      m_stageJoinResponseInboxJson =
          reinterpret_cast<RuntimeStageJoinResponseInboxResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_stage_join_response_inbox_result_json"));
      m_queueJoinResponseOutboxJson =
          reinterpret_cast<RuntimeQueueJoinResponseOutboxResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_queue_join_response_outbox_result_json"));
      m_listJoinResponseOutboxJson =
          reinterpret_cast<RuntimeListJoinResponseOutboxResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_list_join_response_outbox_result_json"));
      m_listDueJoinResponseOutboxJson =
          reinterpret_cast<RuntimeListJoinResponseOutboxResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_list_due_join_response_outbox_result_json"));
      m_submitJoinResponseOutboxEntryDirectJson =
          reinterpret_cast<RuntimeSubmitJoinResponseOutboxEntryDirectResultJsonFn>(
              m_library.resolve(
                  "chaft_runtime_submit_join_response_outbox_entry_direct_result_"
                  "json"));
      m_stopDirectPeerJson =
          reinterpret_cast<RuntimeStopDirectPeerResultJsonFn>(
              m_library.resolve("chaft_runtime_stop_direct_peer_result_json"));

      m_ffiReady =
          m_freeString != nullptr &&
          (m_runtimeSnapshotJson != nullptr ||
           m_runtimeSnapshotLatestJson != nullptr) &&
          m_deviceIdJson != nullptr && m_createWorkspaceJson != nullptr &&
          m_createWorkspaceWithAccessPolicyJson != nullptr &&
          (m_listWorkspacesJson != nullptr ||
           m_listWorkspacePageJson != nullptr) &&
          m_createChannelJson != nullptr && m_sendMessageJson != nullptr &&
          m_updateDeviceProfileJson != nullptr &&
          m_updateLocalPersonProfileJson != nullptr &&
          m_publishDeviceKeyPackageJson != nullptr &&
          m_sendAttachmentJson != nullptr && m_saveAttachmentJson != nullptr &&
          m_pruneBlobsJson != nullptr && m_editMessageJson != nullptr &&
          m_deleteMessageJson != nullptr && m_addReactionJson != nullptr &&
          m_removeReactionJson != nullptr && m_markChannelReadJson != nullptr &&
          m_inviteMemberJson != nullptr &&
          m_createWorkspaceInviteJson != nullptr &&
          m_prepareWorkspaceInviteClaimJson != nullptr &&
          m_claimWorkspaceInviteJson != nullptr &&
          m_importWorkspaceInviteResponseJson != nullptr &&
          m_recordWorkspaceInviteJson != nullptr &&
          m_resolveWorkspaceInviteJson != nullptr &&
          m_recordWorkspaceJoinRequestJson != nullptr &&
          m_recordWorkspaceJoinRequestWithResponseRouteJson != nullptr &&
          m_resolveWorkspaceJoinRequestJson != nullptr &&
          m_updateMemberRoleJson != nullptr &&
          m_updateWorkspaceAccessPolicyJson != nullptr &&
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
          m_submitJoinRequestDirectJson != nullptr &&
          m_pullJoinRequestsDirectJson != nullptr &&
          m_pullJoinResponsesDirectJson != nullptr &&
          m_startDirectPeerJson != nullptr &&
          m_listJoinRequestInboxJson != nullptr &&
          m_ackJoinRequestInboxEntryJson != nullptr &&
          m_queueJoinRequestOutboxJson != nullptr &&
          m_listJoinRequestOutboxJson != nullptr &&
          m_listDueJoinRequestOutboxJson != nullptr &&
          m_submitJoinRequestOutboxEntryDirectJson != nullptr &&
          m_listJoinResponseInboxJson != nullptr &&
          m_ackJoinResponseInboxEntryJson != nullptr &&
          m_stageJoinResponseInboxJson != nullptr &&
          m_queueJoinResponseOutboxJson != nullptr &&
          m_listJoinResponseOutboxJson != nullptr &&
          m_listDueJoinResponseOutboxJson != nullptr &&
          m_submitJoinResponseOutboxEntryDirectJson != nullptr &&
          m_stopDirectPeerJson != nullptr;
      if (m_ffiReady) {
        return;
      }

      m_library.unload();
    }

    setSyncStatus(QStringLiteral("local service unavailable"));
  }

  bool ensureFfiReady() {
    if (m_ffiReady) {
      return true;
    }
    setSyncStatus(QStringLiteral("local service unavailable"));
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
      setSyncStatus(QStringLiteral("workspace data unavailable"));
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
        m_eventStorePath, QStringLiteral("local history path"), false);
  }

  bool ensureRuntimeAccessReady() {
    if (!ensureFfiReady()) {
      return false;
    }
    if (m_runtimeAccessSuspendedUntilUnlock) {
      setSyncStatus(QStringLiteral("workspace locked"));
      return false;
    }
    if (m_runtimeUnlockRequired) {
      setSyncStatus(QStringLiteral("passphrase required"));
      return false;
    }
    if (m_rawEventStoreMode) {
      setSyncStatus(QStringLiteral("open a workspace to do this"));
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
      setSyncStatus(QStringLiteral("workspace data unavailable"));
      return false;
    }

    QString workspaceId;
    if (!selectedWorkspaceIdForDispatch(
            &workspaceId, false, QStringLiteral("workspace required"))) {
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
    setSyncStatus(QStringLiteral("refreshing security..."));
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
    setSyncStatus(QStringLiteral("refreshing security..."));
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
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("room ID"),
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
    setSyncStatus(QStringLiteral("refreshing security..."));
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
      setSyncStatus(QStringLiteral("room required"));
      return false;
    }
    QString metadataError;
    if (!validateMetadataTextForWrite(normalizedChannelId, kMaxChannelIdBytes,
                                      QStringLiteral("room ID"),
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
    setSyncStatus(QStringLiteral("refreshing security..."));
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
    pruneWorkspaceInviteArtifacts(m_workspaceSnapshot);
    emit workspaceSnapshotChanged();
    if (!m_rawEventStoreMode) {
      queueWorkspaceSummariesRefresh();
      queuePublishQueueRefresh();
      queueWorkspaceStorageHealthRefresh();
      refreshActiveSearch();
    }
    if (updateStatus) {
      setSyncStatus(QStringLiteral("messages ready"));
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
    snapshot.insert(QStringLiteral("accessPolicy"), QStringLiteral("invite_only"));
    snapshot.insert(QStringLiteral("channels"), QVariantList{});
    snapshot.insert(QStringLiteral("channelCount"), 0);
    snapshot.insert(QStringLiteral("profiles"), QVariantList{});
    snapshot.insert(QStringLiteral("members"), QVariantList{});
    snapshot.insert(QStringLiteral("invites"), QVariantList{});
    snapshot.insert(QStringLiteral("inviteCount"), 0);
    snapshot.insert(QStringLiteral("joinRequests"), QVariantList{});
    snapshot.insert(QStringLiteral("joinRequestCount"), 0);
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
            &workspaceId, false, QStringLiteral("workspace required"))) {
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
            &workspaceId, false, QStringLiteral("workspace required"))) {
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
            &workspaceId, false, QStringLiteral("workspace required"))) {
      clearWorkspaceStorageHealth();
      return;
    }
    const auto generation = ++m_workspaceStorageHealthGeneration;
    runWorkspaceStorageHealthRefresh(generation, workspaceId);
  }

  void startRuntimeBackgroundServices() {
    if (m_runtimeBackgroundServicesStarted || m_rawEventStoreMode ||
        m_deviceId.trimmed().isEmpty()) {
      return;
    }
    m_runtimeBackgroundServicesStarted = true;
    queueJoinRequestOutboxDrain();
    queueJoinResponseInboxRefresh(false);
    queueJoinResponseOutboxDrain();
    scheduleJoinRequestOutboxPoll();
    scheduleJoinResponseInboxPoll();
    scheduleJoinResponseOutboxPoll();
    if (backgroundReachabilityEnabled()) {
      startBackgroundReachability();
    }
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

  bool queueAccessEnvelopePullFromPeer(const QString &peerEndpoint,
                                       const QString &workspaceId,
                                       bool pullRequests, bool pullResponses,
                                       bool userInitiated) {
    if (!pullRequests && !pullResponses) {
      return false;
    }
    if (!m_ffiReady || m_freeString == nullptr ||
        m_runtimeDir.trimmed().isEmpty()) {
      if (userInitiated) {
        setSyncStatus(QStringLiteral("access checks unavailable"));
      }
      return false;
    }
    if ((pullRequests && m_pullJoinRequestsDirectJson == nullptr) ||
        (pullResponses && m_pullJoinResponsesDirectJson == nullptr)) {
      if (userInitiated) {
        setSyncStatus(QStringLiteral("direct access checks unavailable"));
      }
      return false;
    }
    if (m_accessEnvelopePullInFlight) {
      if (userInitiated) {
        setSyncStatus(QStringLiteral("already checking access updates"));
      }
      return false;
    }

    const auto normalizedPeerEndpoint = peerEndpoint.trimmed();
    const auto normalizedWorkspaceId = workspaceId.trimmed();
    if (normalizedPeerEndpoint.isEmpty()) {
      if (userInitiated) {
        setSyncStatus(QStringLiteral("admin sharing address required"));
      }
      return false;
    }
    if (normalizedWorkspaceId.isEmpty()) {
      if (userInitiated) {
        setSyncStatus(QStringLiteral("workspace ID required"));
      }
      return false;
    }

    QString metadataError;
    if (!validatePeerEndpointForUse(normalizedPeerEndpoint, &metadataError) ||
        !validateMetadataTextForWrite(
            normalizedWorkspaceId, kMaxWorkspaceIdBytes,
            QStringLiteral("workspace ID"), QStringLiteral("128 bytes"),
            &metadataError)) {
      if (userInitiated) {
        setSyncStatus(metadataError);
      }
      return false;
    }

    setAccessEnvelopePullInFlight(true);
    if (userInitiated) {
      setSyncStatus(QStringLiteral("checking access responses..."));
    }
    runAccessEnvelopePullFromPeer(normalizedPeerEndpoint, normalizedWorkspaceId,
                                  pullRequests, pullResponses, userInitiated);
    return true;
  }

  void runAccessEnvelopePullFromPeer(const QString &peerEndpoint,
                                     const QString &workspaceId,
                                     bool pullRequests, bool pullResponses,
                                     bool userInitiated) {
    const QPointer<ChaftController> guard(this);
    const auto requestFn = m_pullJoinRequestsDirectJson;
    const auto responseFn = m_pullJoinResponsesDirectJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    auto *thread = QThread::create(
        [guard, requestFn, responseFn, freeString, runtimeDir, peerEndpoint,
         workspaceId, pullRequests, pullResponses, userInitiated]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto peerEndpointBytes = peerEndpoint.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          QString firstError;
          int requestCount = 0;
          int responseCount = 0;

          if (pullRequests && requestFn != nullptr) {
            QString requestError;
            const auto requestJson = takeWorkerFfiString(
                requestFn(runtimeDirBytes.constData(),
                          peerEndpointBytes.constData(),
                          workspaceIdBytes.constData(),
                          kMaxAccessEnvelopePullEntries),
                freeString, &requestError);
            const auto requestValue =
                requestError.isEmpty()
                    ? resultValueFromWorkerJson(requestJson, &requestError)
                    : QJsonObject();
            if (requestValue.isEmpty()) {
              if (firstError.isEmpty()) {
                firstError = requestError;
              }
            } else {
              requestCount = std::max(
                  0, requestValue.value(QStringLiteral("requestCount")).toInt());
            }
          }

          if (pullResponses && responseFn != nullptr) {
            QString responseError;
            const auto responseJson = takeWorkerFfiString(
                responseFn(runtimeDirBytes.constData(),
                           peerEndpointBytes.constData(),
                           workspaceIdBytes.constData(),
                           kMaxAccessEnvelopePullEntries),
                freeString, &responseError);
            const auto responseValue =
                responseError.isEmpty()
                    ? resultValueFromWorkerJson(responseJson, &responseError)
                    : QJsonObject();
            if (responseValue.isEmpty()) {
              if (firstError.isEmpty()) {
                firstError = responseError;
              }
            } else {
              responseCount = std::max(
                  0,
                  responseValue.value(QStringLiteral("responseCount")).toInt());
            }
          }

          if (guard.isNull()) {
            return;
          }
          QMetaObject::invokeMethod(
              guard.data(),
              [guard, userInitiated, firstError, requestCount,
               responseCount]() {
                if (guard.isNull()) {
                  return;
                }
                guard->setAccessEnvelopePullInFlight(false);
                if (requestCount > 0) {
                  guard->queueJoinRequestInboxRefresh(false);
                }
                if (responseCount > 0) {
                  guard->queueJoinResponseInboxRefresh(userInitiated);
                }
                if (!userInitiated) {
                  return;
                }
                if (!firstError.isEmpty() && requestCount + responseCount == 0) {
                  guard->setSyncStatus(firstError);
                  return;
                }
                if (requestCount + responseCount == 0) {
                  guard->setSyncStatus(
                      QStringLiteral("no access approvals found"));
                }
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  bool queueJoinRequestInboxRefresh(bool userInitiated) {
    if (!hasRuntimeWorkspace()) {
      if (userInitiated) {
        setSyncStatus(QStringLiteral("open a workspace first"));
      }
      return false;
    }
    if (m_joinRequestInboxInFlight) {
      if (userInitiated) {
        setSyncStatus(QStringLiteral("already checking access requests"));
      }
      return false;
    }
    if (m_listJoinRequestInboxJson == nullptr ||
        m_ackJoinRequestInboxEntryJson == nullptr ||
        m_recordWorkspaceJoinRequestJson == nullptr ||
        m_claimWorkspaceInviteJson == nullptr ||
        m_stageJoinResponseInboxJson == nullptr) {
      if (userInitiated) {
        setSyncStatus(QStringLiteral("direct access requests unavailable"));
      }
      return false;
    }

    QString workspaceId;
    if (!selectedWorkspaceIdForDispatch(
            &workspaceId, false, QStringLiteral("workspace required"))) {
      return false;
    }

    setJoinRequestInboxInFlight(true);
    if (userInitiated) {
      setSyncStatus(QStringLiteral("checking access requests..."));
    }
    const auto generation = ++m_joinRequestInboxGeneration;
    runJoinRequestInboxRefresh(workspaceId, generation, userInitiated);
    return true;
  }

  void scheduleJoinRequestInboxPoll() {
    if (!peerHosting()) {
      return;
    }
    QTimer::singleShot(kJoinRequestInboxPollMs, this, [this]() {
      if (!peerHosting()) {
        return;
      }
      queueJoinRequestInboxRefresh(false);
      scheduleJoinRequestInboxPoll();
    });
  }

  bool queueJoinRequestOutboxDrain() {
    if (!m_ffiReady || m_freeString == nullptr ||
        m_listDueJoinRequestOutboxJson == nullptr ||
        m_submitJoinRequestOutboxEntryDirectJson == nullptr ||
        m_runtimeDir.trimmed().isEmpty()) {
      return false;
    }
    if (m_joinRequestOutboxInFlight || m_joinRequestSubmitInFlight) {
      return false;
    }
    setJoinRequestOutboxInFlight(true);
    runJoinRequestOutboxDrain();
    return true;
  }

  void scheduleJoinRequestOutboxPoll() {
    if (!m_ffiReady || m_rawEventStoreMode) {
      return;
    }
    QTimer::singleShot(kJoinRequestOutboxPollMs, this, [this]() {
      if (!m_ffiReady || m_rawEventStoreMode) {
        return;
      }
      queueJoinRequestOutboxDrain();
      scheduleJoinRequestOutboxPoll();
    });
  }

  bool queueJoinResponseInboxRefresh(bool userInitiated) {
    if (!m_ffiReady || m_freeString == nullptr ||
        m_listJoinResponseInboxJson == nullptr ||
        m_runtimeDir.trimmed().isEmpty()) {
      if (userInitiated) {
        setSyncStatus(QStringLiteral("access responses unavailable"));
      }
      return false;
    }
    if (m_joinResponseInboxInFlight) {
      if (userInitiated) {
        setSyncStatus(QStringLiteral("already checking access responses"));
      }
      return false;
    }
    setJoinResponseInboxInFlight(true);
    if (userInitiated) {
      setSyncStatus(QStringLiteral("checking access responses..."));
    }
    runJoinResponseInboxRefresh(userInitiated);
    return true;
  }

  void scheduleJoinResponseInboxPoll() {
    if (!m_ffiReady || m_rawEventStoreMode) {
      return;
    }
    QTimer::singleShot(kJoinResponseInboxPollMs, this, [this]() {
      if (!m_ffiReady || m_rawEventStoreMode) {
        return;
      }
      queueJoinResponseInboxRefresh(false);
      scheduleJoinResponseInboxPoll();
    });
  }

  bool queueJoinResponseOutboxDrain() {
    if (!m_ffiReady || m_freeString == nullptr ||
        m_listDueJoinResponseOutboxJson == nullptr ||
        m_submitJoinResponseOutboxEntryDirectJson == nullptr ||
        m_runtimeDir.trimmed().isEmpty()) {
      return false;
    }
    if (m_joinResponseOutboxInFlight) {
      return false;
    }
    setJoinResponseOutboxInFlight(true);
    runJoinResponseOutboxDrain();
    return true;
  }

  void scheduleJoinResponseOutboxPoll() {
    if (!m_ffiReady || m_rawEventStoreMode) {
      return;
    }
    QTimer::singleShot(kJoinResponseOutboxPollMs, this, [this]() {
      if (!m_ffiReady || m_rawEventStoreMode) {
        return;
      }
      queueJoinResponseOutboxDrain();
      scheduleJoinResponseOutboxPoll();
    });
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
            &workspaceId, false, QStringLiteral("workspace required"))) {
      return;
    }
    if (m_storeSnapshotJson == nullptr &&
        m_storeSnapshotLatestJson == nullptr) {
      setSyncStatus(QStringLiteral("local history preview unavailable"));
      return;
    }

    const auto generation = ++m_runtimeWriteGeneration;
    runStoreSnapshotHydration(generation, workspaceId);
  }

  void persistDesktopConfig() {
    saveDesktopConfig(m_runtimeDir, m_workspaceId, m_defaultPeerEndpoint,
                      m_backupPeerEndpoints, m_backupPeerStatuses,
                      m_autoBackupEnabled, m_themeId, m_themeMode,
                      m_darkThemeId, m_lightThemeId, m_inspectorPinned,
                      m_reducedMotionEnabled, m_notificationsEnabled,
                      m_notificationSoundEnabled, m_notificationPreviewEnabled,
                      m_externalLinkConfirmationEnabled,
                      m_mutedChannels,
                      m_composerDrafts,
                      m_pendingJoinRequests,
                      m_workspaceInviteArtifacts,
                      m_windowGeometry);
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
                        QStringLiteral(
                            "backup partial, %1 history item(s) to fetch")
                            .arg(skippedGapCount));
        } else {
          status.insert(QStringLiteral("lastPartial"), false);
          status.insert(QStringLiteral("lastMessage"),
                        QStringLiteral("backup files repaired"));
        }
      } else {
        status.insert(QStringLiteral("lastMissingBlobCount"), remaining);
        const auto message =
            skippedGapCount > 0
                ? QStringLiteral(
                      "backup partial, %1 file(s) to retry, %2 history "
                      "item(s) to fetch")
                      .arg(remaining)
                      .arg(skippedGapCount)
                : QStringLiteral("backup partial, %1 file(s) to retry")
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
                                 .toString(QStringLiteral("file retry failed"));
        failure.insert(QStringLiteral("message"), message);
      }
      peerFailures.insert(peerEndpoint, failure);
    }

    for (auto it = peerFailures.cbegin(); it != peerFailures.cend(); ++it) {
      const auto failure = it.value().toMap();
      recordBackupResult(
          it.key(), false,
          variantStringValue(failure.value(QStringLiteral("message")),
                             QStringLiteral("file retry failed")),
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
      setSyncStatus(QStringLiteral("backup address required"));
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
                    QStringLiteral("sync finished after switching workspaces"));
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
                      "shared %1 update(s), %2 file(s), %3 file(s) to retry, "
                      "%4 history item(s) to fetch")
                      .arg(publishedCount)
                      .arg(publishedBlobCount)
                      .arg(publishedMissingBlobCount)
                      .arg(publishedSkippedGapCount));
            } else if (mode == DirectSyncMode::Backup) {
              const auto message =
                  QStringLiteral(
                      "backed up %1 update(s), %2 file(s), %3 file(s) to "
                      "retry, %4 history item(s) to fetch")
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
                  QStringLiteral("fetched %1 update(s), %2 file(s), %3 "
                                 "file(s) to retry, %4 history item(s) to "
                                 "fetch, %5 access item(s)")
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
                  QStringLiteral("synced %1 update(s), %2 file(s), %3 "
                                 "file(s) to retry, %4 history item(s) to "
                                 "fetch shared / %5 update(s), %6 file(s), "
                                 "%7 file(s) to retry, %8 history item(s) "
                                 "to fetch, %9 access item(s) fetched")
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
            if (mode == DirectSyncMode::Pull || mode == DirectSyncMode::Sync) {
              guard->queueAccessEnvelopePullFromPeer(peerEndpoint, workspaceId,
                                                     true, true, false);
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
              guard->setSyncStatus(QStringLiteral("no pending file transfers"));
              return;
            }

            guard->recordBackupPeerErrorsFromRetry(value);
            guard->reconcileBackupPeerPartialStateFromRetry(value);
            guard->setSyncStatus(
                QStringLiteral("retried %1 file(s), repaired %2, missing %3, "
                               "%4 device error(s)")
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
                QStringLiteral("cleaned up %1 file object(s)").arg(removedCount));
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
                  QStringLiteral("room list did not contain rooms"));
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
              guard->setSyncStatus(QStringLiteral("room list is stale"));
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
                  QStringLiteral("loaded %1 room(s)").arg(appended));
            } else {
              guard->setSyncStatus(QStringLiteral("all rooms loaded"));
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
                  QStringLiteral("room lookup did not contain rows"));
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
                  QStringLiteral("loaded room %1").arg(channelId));
            } else {
              guard->setSyncStatus(QStringLiteral("room lookup was empty"));
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
            guard->startRuntimeBackgroundServices();

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
                guard->setSyncStatus(QStringLiteral("messages ready, ") +
                                     compromiseSummary);
              } else if (!compromiseError.isEmpty()) {
                guard->setSyncStatus(QStringLiteral("security check failed: ") +
                                     compromiseError);
              }
              guard->queueJoinRequestInboxRefresh(false);
              return;
            }

            if (!snapshotError.isEmpty()) {
              guard->setSyncStatus(snapshotError);
            } else if (!deviceError.isEmpty()) {
              guard->setSyncStatus(deviceError);
            } else if (guard->m_workspaceId.isEmpty()) {
              guard->setSyncStatus(QStringLiteral("create a workspace first"));
            } else {
              guard->setSyncStatus(QStringLiteral("workspace ready"));
            }
            guard->queueJoinRequestInboxRefresh(false);
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
                  "history fixed after switching workspaces"));
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
                    "fixed %1 history issue(s), restored %2, cleared %3")
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
            guard->setSyncStatus(QStringLiteral("local history preview"));
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
                  "history loaded after switching workspaces"));
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
                generation, QStringLiteral("access details shared"),
                QStringLiteral("access details shared after switching workspaces"));
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
                QStringLiteral("sharing address updated after switching workspaces"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceCreate(const QString &workspaceName,
                          const QString &channelName,
                          const QString &accessPolicy,
                          const QString &previousWorkspaceId,
                          bool hadRuntimeWorkspace, quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto createFn = m_createWorkspaceWithAccessPolicyJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, createFn, snapshotFn,
                                    snapshotLatestFn, freeString, runtimeDir,
                                    identityFile, workspaceName, channelName,
                                    accessPolicy, previousWorkspaceId,
                                    hadRuntimeWorkspace, generation,
                                    timelineLimit]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      const auto identityFileBytes = identityFile.toUtf8();
      const auto workspaceNameBytes = workspaceName.toUtf8();
      const auto channelNameBytes = channelName.toUtf8();
      const auto accessPolicyBytes = accessPolicy.toUtf8();
      char *raw = createFn(
          runtimeDirBytes.constData(),
          identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
          workspaceNameBytes.constData(), channelNameBytes.constData(),
          accessPolicyBytes.constData());

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
                  QStringLiteral("workspace created after switching workspaces"));
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
            if (guard->peerHosting()) {
              guard->refreshHostedPeerEndpointHint();
            }
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
        *error = QStringLiteral("local service returned no data");
      }
      return {};
    }
    return resultValueFromJson(json, error);
  }

  static QByteArray takeWorkerFfiString(char *raw, FreeStringFn freeString,
                                        QString *error = nullptr) {
    return takeBoundedFfiString(raw, freeString, kMaxDesktopFfiJsonBytes,
                                QStringLiteral("local service worker"),
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
            ? QStringLiteral("sharing address %1")
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
                           QStringLiteral("sharing address expired"),
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
              if (guard->m_backgroundReachabilityFallbackPending) {
                guard->scheduleBackgroundReachabilityRetry(error);
                return;
              }
              guard->setSyncStatus(error);
              return;
            }
            if (peerId.isEmpty() || endpoint.isEmpty()) {
              if (guard->m_backgroundReachabilityFallbackPending) {
                guard->scheduleBackgroundReachabilityRetry(
                    QStringLiteral(
                        "hosting did not return a sharing address"));
                return;
              }
              guard->setSyncStatus(
                  QStringLiteral("hosting did not return a sharing address"));
              return;
            }

            if (guard->m_backgroundReachabilityFallbackPending) {
              guard->m_backgroundReachabilityFallbackPending = false;
              guard->m_backgroundReachabilityRetryAttempt = 0;
            }
            guard->m_hostedPeerId = peerId;
            guard->m_hostedPeerEndpoint = endpoint;
            guard->m_hostedPeerEndpointId = QStringLiteral("hosted-direct");
            guard->m_hostedPeerTransport = QStringLiteral("direct-tcp");
            emit guard->hostedPeerChanged();
            guard->setSyncStatus(
                QStringLiteral("sharing address %1").arg(endpoint));
            guard->publishHostedPeerEndpoint(
                guard->m_hostedPeerEndpointId, endpoint,
                guard->m_hostedPeerTransport,
                QStringLiteral("sharing address %1").arg(endpoint));
            guard->queueJoinRequestInboxRefresh(false);
            guard->scheduleJoinRequestInboxPoll();
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
              if (guard->m_backgroundReachabilityFallbackPending &&
                  developmentLoopbackFallbackEnabled() &&
                  guard->m_startDirectPeerJson != nullptr) {
                guard->setSyncStatus(
                    QStringLiteral("using local peer access..."));
                guard->runDirectPeerStart(QStringLiteral("127.0.0.1:0"));
                return;
              }
              if (guard->m_backgroundReachabilityFallbackPending) {
                guard->scheduleBackgroundReachabilityRetry(error);
                return;
              }
              guard->m_backgroundReachabilityFallbackPending = false;
              guard->setSyncStatus(error);
              return;
            }
            if (peerId.isEmpty() || endpoint.isEmpty()) {
              if (guard->m_backgroundReachabilityFallbackPending &&
                  developmentLoopbackFallbackEnabled() &&
                  guard->m_startDirectPeerJson != nullptr) {
                guard->setSyncStatus(
                    QStringLiteral("using local peer access..."));
                guard->runDirectPeerStart(QStringLiteral("127.0.0.1:0"));
                return;
              }
              if (guard->m_backgroundReachabilityFallbackPending) {
                guard->scheduleBackgroundReachabilityRetry(QStringLiteral(
                    "relay did not return a sharing address"));
                return;
              }
              guard->m_backgroundReachabilityFallbackPending = false;
              guard->setSyncStatus(
                  QStringLiteral("relay did not return a sharing address"));
              return;
            }

            guard->m_backgroundReachabilityFallbackPending = false;
            guard->m_backgroundReachabilityRetryAttempt = 0;
            guard->m_hostedPeerId = peerId;
            guard->m_hostedPeerEndpoint = endpoint;
            guard->m_hostedPeerEndpointId = QStringLiteral("hosted-iroh");
            guard->m_hostedPeerTransport = QStringLiteral("iroh");
            emit guard->hostedPeerChanged();
            guard->setSyncStatus(
                QStringLiteral("sharing address %1").arg(endpoint));
            guard->publishHostedPeerEndpoint(
                guard->m_hostedPeerEndpointId, endpoint,
                guard->m_hostedPeerTransport,
                QStringLiteral("sharing address %1").arg(endpoint));
            guard->queueJoinRequestInboxRefresh(false);
            guard->scheduleJoinRequestInboxPoll();
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
                    ? QStringLiteral("device no longer reachable")
                    : QStringLiteral("stopped sharing %1").arg(endpoint));
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
                  QStringLiteral("room access refreshed after switching workspaces"));
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
            guard->setSyncStatus(QStringLiteral("room access refreshed"));
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
                  QStringLiteral("workspace access refreshed after switching workspaces"));
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
            guard->setSyncStatus(QStringLiteral("workspace access refreshed"));
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
                  "security review finished after switching workspaces"));
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
                  QStringLiteral("workspace import returned no workspace"));
              return;
            }

            guard->applyWorkspaceSummariesResult(summaries, summariesError);
            if (guard->m_workspaceId != previousWorkspaceId) {
              guard->setSyncStatus(successStatus +
                                   QStringLiteral(" after switching workspaces"));
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
                      "room access import returned no workspace"));
                  return;
                }

                guard->applyWorkspaceSummariesResult(summaries, summariesError);
                if (guard->m_workspaceId != previousWorkspaceId) {
                  guard->setSyncStatus(QStringLiteral(
                      "room access imported after switching workspaces"));
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
                guard->setSyncStatus(QStringLiteral("room access imported"));
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
      const auto importedChannelCount =
          value.value(QStringLiteral("importedChannelCount")).toInt();
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
           hadRuntimeWorkspace, generation, importedChannelCount]() {
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
              guard->setLastRecoveryImportedChannelCount(0);
              guard->setSyncStatus(error);
              return;
            }
            if (importedWorkspaceId.isEmpty()) {
              guard->setLastRecoveryImportedChannelCount(0);
              guard->setSyncStatus(
                  QStringLiteral("recovery import returned no workspace"));
              return;
            }

            guard->applyWorkspaceSummariesResult(summaries, summariesError);
            if (guard->m_workspaceId != previousWorkspaceId) {
              guard->setLastRecoveryImportedChannelCount(importedChannelCount);
              guard->setSyncStatus(
                  QStringLiteral("recovery kit imported after workspace "
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
            guard->setLastRecoveryImportedChannelCount(importedChannelCount);
            guard->setSyncStatus(QStringLiteral("recovery kit imported"));
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
                  "search refreshed after switching workspaces"));
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(status.isEmpty()
                                       ? QStringLiteral("search refresh failed")
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
                QStringLiteral("access refreshed after switching workspaces"));
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
                QStringLiteral("access refreshed after switching workspaces"));
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
                QStringLiteral("access refreshed after switching workspaces"));
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
                QStringLiteral("access refreshed after switching workspaces"));
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
                QStringLiteral("shared message support info: %1 update(s), "
                               "%2 file(s), %3 file(s) to retry")
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
                  QStringLiteral("room created after switching workspaces"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setLastCreatedChannelId(
                value.value(QStringLiteral("channelId")).toString());
            guard->setSyncStatus(QStringLiteral("room created"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelDetailsUpdate(const QString &channelId,
                               const QString &channelName,
                               const QString &channelTopic,
                               quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto updateFn = m_updateChannelDetailsJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, updateFn, snapshotFn, snapshotLatestFn, freeString, runtimeDir,
         identityFile, workspaceId, channelId, channelName, channelTopic,
         generation, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto channelIdBytes = channelId.toUtf8();
          const auto channelNameBytes = channelName.toUtf8();
          const auto channelTopicBytes = channelTopic.toUtf8();

          QString error;
          const auto json = takeFfiString(
              updateFn(runtimeDirBytes.constData(),
                       identityFileBytes.isEmpty()
                           ? nullptr
                           : identityFileBytes.constData(),
                       workspaceIdBytes.constData(), channelIdBytes.constData(),
                       channelNameBytes.constData(),
                       channelTopicBytes.constData()),
              freeString, &error);
          const auto value =
              error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
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
                if (workspaceId != guard->m_workspaceId ||
                    generation < guard->m_lastAppliedRuntimeWriteGeneration) {
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

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setSyncStatus(QStringLiteral("room details saved"));
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runChannelArchiveUpdate(const QString &channelId, bool archived,
                               quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto updateFn = m_updateChannelArchiveJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, updateFn, snapshotFn, snapshotLatestFn, freeString, runtimeDir,
         identityFile, workspaceId, channelId, archived, generation,
         timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto channelIdBytes = channelId.toUtf8();

          QString error;
          const auto json = takeFfiString(
              updateFn(runtimeDirBytes.constData(),
                       identityFileBytes.isEmpty()
                           ? nullptr
                           : identityFileBytes.constData(),
                       workspaceIdBytes.constData(), channelIdBytes.constData(),
                       archived),
              freeString, &error);
          const auto value =
              error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
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
               archived, generation]() {
                if (guard.isNull()) {
                  return;
                }

                guard->setSyncInFlight(false);
                if (workspaceId != guard->m_workspaceId ||
                    generation < guard->m_lastAppliedRuntimeWriteGeneration) {
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

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setSyncStatus(archived
                                         ? QStringLiteral("room archived")
                                         : QStringLiteral("room restored"));
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runDirectMessageCreate(const QString &channelName,
                              const QString &memberDeviceId,
                              quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto createFn = m_createDirectMessageChannelJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, createFn, snapshotFn, snapshotLatestFn, freeString,
         runtimeDir, identityFile, workspaceId, channelName, memberDeviceId,
         generation, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto channelNameBytes = channelName.toUtf8();
          const auto memberDeviceIdBytes = memberDeviceId.toUtf8();

          QString error;
          const auto createJson = takeFfiString(
              createFn(runtimeDirBytes.constData(),
                       identityFileBytes.isEmpty()
                           ? nullptr
                           : identityFileBytes.constData(),
                       workspaceIdBytes.constData(),
                       channelNameBytes.constData(),
                       memberDeviceIdBytes.constData()),
              freeString, &error);
          const auto createdValue = error.isEmpty()
                                        ? resultValueFromJson(createJson, &error)
                                        : QJsonObject();
          const auto channelId =
              createdValue.value(QStringLiteral("channelId")).toString();
          QJsonObject snapshotValue;
          QString snapshotError;
          if (error.isEmpty()) {
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
              [guard, error, snapshotValue, snapshotError, workspaceId,
               channelId, generation]() {
                if (guard.isNull()) {
                  return;
                }
                guard->setSyncInFlight(false);
                if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
                  guard->queueRuntimeSnapshotRefreshIfCurrent(
                      error.isEmpty(), workspaceId);
                  return;
                }
                if (!error.isEmpty()) {
                  guard->setSyncStatus(error);
                  return;
                }
                if (snapshotValue.isEmpty()) {
                  guard->setSyncStatus(snapshotError);
                  return;
                }
                if (guard->m_workspaceId != workspaceId) {
                  guard->setSyncStatus(QStringLiteral(
                      "direct message created after switching workspaces"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setLastCreatedChannelId(channelId);
                guard->setSyncStatus(QStringLiteral("direct message ready"));
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runDeviceProfileUpdate(const QString &displayName, quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto updateFn = m_updateDeviceProfileJson;
    const auto personUpdateFn = m_updateLocalPersonProfileJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create([guard, updateFn, personUpdateFn, snapshotFn,
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
      auto value =
          error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
      if (!value.isEmpty()) {
        char *personRaw =
            personUpdateFn(runtimeDirBytes.constData(),
                           identityFileBytes.isEmpty()
                               ? nullptr
                               : identityFileBytes.constData(),
                           workspaceIdBytes.constData(),
                           displayNameBytes.constData());
        QString personError;
        const auto personJson = takeFfiString(personRaw, freeString, &personError);
        const auto personValue = personError.isEmpty()
                                     ? resultValueFromJson(personJson, &personError)
                                     : QJsonObject();
        if (personValue.isEmpty()) {
          value = QJsonObject();
          error = personError.isEmpty()
                      ? QStringLiteral("name could not be saved")
                      : personError;
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
                  QStringLiteral("name saved after switching workspaces"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("name saved"));
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
                  QStringLiteral("invite recorded after switching workspaces"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("invite recorded"));
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceJoinRequestRecord(const QString &requestId,
                                     const QString &deviceId,
                                     const QString &displayName,
                                     const QString &note,
                                     const QString &sourceType,
                                     const QString &sourceInviteId,
                                     const QString &sourceDisplayName,
                                     const QString &sourceApprovalPolicy,
                                     const QString &responsePeerEndpoint,
                                     quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto recordFn = m_recordWorkspaceJoinRequestJson;
    const auto recordWithResponseRouteFn =
        m_recordWorkspaceJoinRequestWithResponseRouteJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, recordFn, recordWithResponseRouteFn, snapshotFn,
         snapshotLatestFn, freeString, runtimeDir, identityFile, workspaceId,
         requestId, deviceId, displayName, note, sourceType, sourceInviteId,
         sourceDisplayName, sourceApprovalPolicy, responsePeerEndpoint,
         generation, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto requestIdBytes = requestId.toUtf8();
          const auto deviceIdBytes = deviceId.toUtf8();
          const auto displayNameBytes = displayName.toUtf8();
          const auto noteBytes = note.toUtf8();
          const auto sourceTypeBytes = sourceType.toUtf8();
          const auto sourceInviteIdBytes = sourceInviteId.toUtf8();
          const auto sourceDisplayNameBytes = sourceDisplayName.toUtf8();
          const auto sourceApprovalPolicyBytes = sourceApprovalPolicy.toUtf8();
          const auto responsePeerEndpointBytes = responsePeerEndpoint.toUtf8();
          char *raw = recordWithResponseRouteFn != nullptr
                          ? recordWithResponseRouteFn(
                                runtimeDirBytes.constData(),
                                identityFileBytes.isEmpty()
                                    ? nullptr
                                    : identityFileBytes.constData(),
                                workspaceIdBytes.constData(),
                                requestIdBytes.constData(),
                                deviceIdBytes.constData(),
                                displayNameBytes.constData(),
                                noteBytes.constData(), sourceTypeBytes.constData(),
                                sourceInviteIdBytes.constData(),
                                sourceDisplayNameBytes.constData(),
                                sourceApprovalPolicyBytes.constData(),
                                responsePeerEndpointBytes.isEmpty()
                                    ? nullptr
                                    : responsePeerEndpointBytes.constData())
                          : recordFn(
                                runtimeDirBytes.constData(),
                                identityFileBytes.isEmpty()
                                    ? nullptr
                                    : identityFileBytes.constData(),
                                workspaceIdBytes.constData(),
                                requestIdBytes.constData(),
                                deviceIdBytes.constData(),
                                displayNameBytes.constData(),
                                noteBytes.constData(), sourceTypeBytes.constData(),
                                sourceInviteIdBytes.constData(),
                                sourceDisplayNameBytes.constData(),
                                sourceApprovalPolicyBytes.constData());

          QString error;
          const auto json = takeFfiString(raw, freeString, &error);
          const auto value =
              error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
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
                  guard->setSyncStatus(QStringLiteral(
                      "access request recorded after switching workspaces"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setSyncStatus(QStringLiteral("access request recorded"));
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runJoinRequestInboxRefresh(const QString &workspaceId,
                                  quint64 generation, bool userInitiated) {
    const QPointer<ChaftController> guard(this);
    const auto listFn = m_listJoinRequestInboxJson;
    const auto ackFn = m_ackJoinRequestInboxEntryJson;
    const auto recordFn = m_recordWorkspaceJoinRequestJson;
    const auto recordWithResponseRouteFn =
        m_recordWorkspaceJoinRequestWithResponseRouteJson;
    const auto claimInviteFn = m_claimWorkspaceInviteJson;
    const auto stageResponseFn = m_stageJoinResponseInboxJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, listFn, ackFn, recordFn, recordWithResponseRouteFn,
         claimInviteFn, stageResponseFn, snapshotFn, snapshotLatestFn,
         freeString, runtimeDir, identityFile, workspaceId, generation,
         userInitiated, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();

          QString error;
          const auto listJson = takeFfiString(
              listFn(runtimeDirBytes.constData(), kMaxJoinRequestInboxEntries),
              freeString, &error);
          const auto inboxValue =
              error.isEmpty() ? resultValueFromJson(listJson, &error)
                              : QJsonObject();
          int recordedCount = 0;
          int claimedCount = 0;
          int skippedCount = 0;
          QString firstRecordError;

          if (!inboxValue.isEmpty()) {
            const auto entries =
                inboxValue.value(QStringLiteral("entries")).toArray();
            for (const auto &entryValue : entries) {
              const auto entry = entryValue.toObject();
              const auto entryId =
                  entry.value(QStringLiteral("entryId")).toString().trimmed();
              const auto entryWorkspaceId =
                  entry.value(QStringLiteral("workspaceId")).toString().trimmed();
              const auto requestText =
                  entry.value(QStringLiteral("requestText")).toString().trimmed();
              if (entryId.isEmpty() || requestText.isEmpty()) {
                ++skippedCount;
                continue;
              }

              QJsonParseError parseError;
              const auto requestDocument =
                  QJsonDocument::fromJson(requestText.toUtf8(), &parseError);
              if (parseError.error != QJsonParseError::NoError ||
                  !requestDocument.isObject()) {
                ++skippedCount;
                continue;
              }
              const auto request = requestDocument.object();
              const auto requestKind =
                  request.value(QStringLiteral("kind")).toString().trimmed();
              if (requestKind != QStringLiteral("chaft.workspace-join-request.v1") &&
                  requestKind !=
                      QStringLiteral("chaft.workspace-invite-claim.v1")) {
                ++skippedCount;
                continue;
              }

              auto targetWorkspaceId =
                  request.value(QStringLiteral("workspaceId"))
                      .toString()
                      .trimmed();
              if (targetWorkspaceId.isEmpty()) {
                targetWorkspaceId = entryWorkspaceId;
              }
              if (!targetWorkspaceId.isEmpty() &&
                  targetWorkspaceId != workspaceId) {
                continue;
              }

              if (requestKind ==
                  QStringLiteral("chaft.workspace-invite-claim.v1")) {
                const auto requestTextBytes = requestText.toUtf8();
                QString claimError;
                const auto claimJson = takeFfiString(
                    claimInviteFn(
                        runtimeDirBytes.constData(),
                        identityFileBytes.isEmpty()
                            ? nullptr
                            : identityFileBytes.constData(),
                        requestTextBytes.constData()),
                    freeString, &claimError);
                const auto claimValue =
                    claimError.isEmpty()
                        ? resultValueFromJson(claimJson, &claimError)
                        : QJsonObject();
                const auto response =
                    claimValue.value(QStringLiteral("response")).toObject();
                if (claimValue.isEmpty() || response.isEmpty()) {
                  if (firstRecordError.isEmpty()) {
                    firstRecordError = claimError;
                  }
                  continue;
                }
                const auto responseBytes =
                    QJsonDocument(response).toJson(QJsonDocument::Compact);
                QString stageError;
                const auto stageJson = takeFfiString(
                    stageResponseFn(runtimeDirBytes.constData(),
                                    workspaceIdBytes.constData(),
                                    responseBytes.constData()),
                    freeString, &stageError);
                const auto stageValue =
                    stageError.isEmpty()
                        ? resultValueFromJson(stageJson, &stageError)
                        : QJsonObject();
                if (stageValue.isEmpty()) {
                  if (firstRecordError.isEmpty()) {
                    firstRecordError = stageError;
                  }
                  continue;
                }

                const auto entryIdBytes = entryId.toUtf8();
                QString ackError;
                const auto ackJson = takeFfiString(
                    ackFn(runtimeDirBytes.constData(), entryIdBytes.constData()),
                    freeString, &ackError);
                const auto ackValue =
                    ackError.isEmpty()
                        ? resultValueFromJson(ackJson, &ackError)
                        : QJsonObject();
                if (ackValue.isEmpty()) {
                  if (firstRecordError.isEmpty()) {
                    firstRecordError = ackError;
                  }
                  continue;
                }
                ++recordedCount;
                ++claimedCount;
                continue;
              }

              auto requestId =
                  request.value(QStringLiteral("requestId")).toString().trimmed();
              if (requestId.isEmpty()) {
                requestId = entryId;
              }
              const auto deviceId =
                  request.value(QStringLiteral("deviceId")).toString().trimmed();
              if (deviceId.isEmpty()) {
                ++skippedCount;
                continue;
              }
              const auto displayName =
                  request.value(QStringLiteral("displayName"))
                      .toString()
                      .trimmed();
              const auto note =
                  request.value(QStringLiteral("note")).toString().trimmed();
              auto sourceType =
                  request.value(QStringLiteral("sourceType"))
                      .toString()
                      .trimmed();
              if (sourceType.isEmpty()) {
                sourceType = QStringLiteral("direct_peer");
              }
              const auto sourceInviteId =
                  request.value(QStringLiteral("sourceInviteId"))
                      .toString()
                      .trimmed();
              const auto sourceDisplayName =
                  request.value(QStringLiteral("sourceDisplayName"))
                      .toString()
                      .trimmed();
              const auto sourceApprovalPolicy =
                  request.value(QStringLiteral("sourceApprovalPolicy"))
                      .toString()
                      .trimmed();
              const auto responsePeerEndpoint =
                  request.value(QStringLiteral("responsePeerEndpoint"))
                      .toString()
                      .trimmed();

              const auto requestIdBytes = requestId.toUtf8();
              const auto deviceIdBytes = deviceId.toUtf8();
              const auto displayNameBytes = displayName.toUtf8();
              const auto noteBytes = note.toUtf8();
              const auto sourceTypeBytes = sourceType.toUtf8();
              const auto sourceInviteIdBytes = sourceInviteId.toUtf8();
              const auto sourceDisplayNameBytes = sourceDisplayName.toUtf8();
              const auto sourceApprovalPolicyBytes =
                  sourceApprovalPolicy.toUtf8();
              const auto responsePeerEndpointBytes =
                  responsePeerEndpoint.toUtf8();
              QString recordError;
              const auto recordJson = takeFfiString(
                  recordWithResponseRouteFn != nullptr
                      ? recordWithResponseRouteFn(
                            runtimeDirBytes.constData(),
                            identityFileBytes.isEmpty()
                                ? nullptr
                                : identityFileBytes.constData(),
                            workspaceIdBytes.constData(),
                            requestIdBytes.constData(), deviceIdBytes.constData(),
                            displayNameBytes.constData(), noteBytes.constData(),
                            sourceTypeBytes.constData(),
                            sourceInviteIdBytes.constData(),
                            sourceDisplayNameBytes.constData(),
                            sourceApprovalPolicyBytes.constData(),
                            responsePeerEndpointBytes.isEmpty()
                                ? nullptr
                                : responsePeerEndpointBytes.constData())
                      : recordFn(runtimeDirBytes.constData(),
                                 identityFileBytes.isEmpty()
                                     ? nullptr
                                     : identityFileBytes.constData(),
                                 workspaceIdBytes.constData(),
                                 requestIdBytes.constData(),
                                 deviceIdBytes.constData(),
                                 displayNameBytes.constData(),
                                 noteBytes.constData(),
                                 sourceTypeBytes.constData(),
                                 sourceInviteIdBytes.constData(),
                                 sourceDisplayNameBytes.constData(),
                                 sourceApprovalPolicyBytes.constData()),
                  freeString, &recordError);
              const auto recordValue =
                  recordError.isEmpty()
                      ? resultValueFromJson(recordJson, &recordError)
                      : QJsonObject();
              if (recordValue.isEmpty()) {
                if (firstRecordError.isEmpty()) {
                  firstRecordError = recordError;
                }
                continue;
              }

              const auto entryIdBytes = entryId.toUtf8();
              QString ackError;
              const auto ackJson = takeFfiString(
                  ackFn(runtimeDirBytes.constData(), entryIdBytes.constData()),
                  freeString, &ackError);
              const auto ackValue =
                  ackError.isEmpty() ? resultValueFromJson(ackJson, &ackError)
                                     : QJsonObject();
              if (ackValue.isEmpty()) {
                if (firstRecordError.isEmpty()) {
                  firstRecordError = ackError;
                }
                continue;
              }

              ++recordedCount;
            }
          }

          QJsonObject snapshotValue;
          QString snapshotError;
          if (recordedCount > 0) {
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
              [guard, workspaceId, generation, userInitiated, inboxValue, error,
               firstRecordError, snapshotValue, snapshotError, recordedCount,
               skippedCount, claimedCount]() {
                if (guard.isNull()) {
                  return;
                }
                guard->setJoinRequestInboxInFlight(false);
                if (generation != guard->m_joinRequestInboxGeneration ||
                    guard->m_workspaceId != workspaceId) {
                  return;
                }

                if (inboxValue.isEmpty()) {
                  if (userInitiated) {
                    guard->setSyncStatus(error);
                  }
                  return;
                }
                if (recordedCount > 0) {
                  if (snapshotValue.isEmpty()) {
                    guard->setSyncStatus(snapshotError);
                    return;
                  }
                  guard->applyRuntimeSnapshot(snapshotValue, false);
                  guard->setSyncStatus(
                      claimedCount > 0
                          ? QStringLiteral("%1 secure invite claim(s) accepted")
                                .arg(claimedCount)
                          : QStringLiteral("%1 access request(s) received")
                                .arg(recordedCount));
                  return;
                }
                if (userInitiated && !firstRecordError.isEmpty()) {
                  guard->setSyncStatus(firstRecordError);
                  return;
                }
                if (userInitiated && skippedCount > 0) {
                  guard->setSyncStatus(
                      QStringLiteral("some access requests need review"));
                  return;
                }
                if (userInitiated) {
                  guard->setSyncStatus(QStringLiteral("no new access requests"));
                }
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceJoinRequestDirectSubmit(const QString &peerEndpoint,
                                           const QString &workspaceId,
                                           const QString &requestJson) {
    setJoinRequestSubmitInFlight(true);
    const QPointer<ChaftController> guard(this);
    const auto submitFn = m_submitJoinRequestDirectJson;
    const auto queueOutboxFn = m_queueJoinRequestOutboxJson;
    const auto submitOutboxFn = m_submitJoinRequestOutboxEntryDirectJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    auto *thread = QThread::create(
        [guard, submitFn, queueOutboxFn, submitOutboxFn, freeString, runtimeDir,
         peerEndpoint, workspaceId, requestJson]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto peerEndpointBytes = peerEndpoint.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto requestJsonBytes = requestJson.toUtf8();
          QString error;
          QJsonObject value;
          if (queueOutboxFn != nullptr && submitOutboxFn != nullptr &&
              !runtimeDirBytes.isEmpty()) {
            const auto queuedJson = takeWorkerFfiString(
                queueOutboxFn(runtimeDirBytes.constData(),
                              peerEndpointBytes.constData(),
                              workspaceIdBytes.isEmpty()
                                  ? nullptr
                                  : workspaceIdBytes.constData(),
                              requestJsonBytes.constData()),
                freeString, &error);
            const auto queuedValue =
                error.isEmpty() ? resultValueFromWorkerJson(queuedJson, &error)
                                : QJsonObject();
            const auto entry = queuedValue.value(QStringLiteral("entry")).toObject();
            const auto entryId =
                entry.value(QStringLiteral("entryId")).toString().trimmed();
            if (error.isEmpty() && entryId.isEmpty()) {
              error = QStringLiteral("queued access request is missing an ID");
            }
            if (error.isEmpty()) {
              const auto entryIdBytes = entryId.toUtf8();
              const auto submittedJson = takeWorkerFfiString(
                  submitOutboxFn(runtimeDirBytes.constData(),
                                 entryIdBytes.constData()),
                  freeString, &error);
              value = error.isEmpty()
                          ? resultValueFromWorkerJson(submittedJson, &error)
                          : QJsonObject();
            }
          } else {
            const auto json = takeWorkerFfiString(
                submitFn(peerEndpointBytes.constData(),
                         workspaceIdBytes.isEmpty()
                             ? nullptr
                             : workspaceIdBytes.constData(),
                         requestJsonBytes.constData()),
                freeString, &error);
            value =
                error.isEmpty() ? resultValueFromWorkerJson(json, &error)
                                : QJsonObject();
          }

          if (guard.isNull()) {
            return;
          }
          QMetaObject::invokeMethod(
              guard.data(),
              [guard, value, error]() {
                if (guard.isNull()) {
                  return;
                }
                guard->setJoinRequestSubmitInFlight(false);
                if (value.isEmpty()) {
                  guard->setSyncStatus(error);
                  emit guard->joinRequestDirectSubmitFinished(false, error);
                  return;
                }
                const auto status = QStringLiteral("access request delivered");
                guard->setSyncStatus(status);
                emit guard->joinRequestDirectSubmitFinished(true, status);
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runJoinRequestOutboxDrain() {
    const QPointer<ChaftController> guard(this);
    const auto listFn = m_listDueJoinRequestOutboxJson;
    const auto submitFn = m_submitJoinRequestOutboxEntryDirectJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    auto *thread = QThread::create(
        [guard, listFn, submitFn, freeString, runtimeDir]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          QString error;
          const auto listJson = takeWorkerFfiString(
              listFn(runtimeDirBytes.constData(), kMaxJoinRequestOutboxEntries),
              freeString, &error);
          const auto listValue =
              error.isEmpty() ? resultValueFromWorkerJson(listJson, &error)
                              : QJsonObject();
          int deliveredCount = 0;
          int attemptedCount = 0;
          if (!listValue.isEmpty()) {
            const auto entries =
                listValue.value(QStringLiteral("entries")).toArray();
            for (const auto &entryValue : entries) {
              if (attemptedCount >= kMaxJoinRequestOutboxDrainBatch) {
                break;
              }
              const auto entry = entryValue.toObject();
              const auto entryId =
                  entry.value(QStringLiteral("entryId")).toString().trimmed();
              const auto peerEndpoint =
                  entry.value(QStringLiteral("peerEndpoint")).toString().trimmed();
              if (entryId.isEmpty() || peerEndpoint.isEmpty()) {
                continue;
              }
              ++attemptedCount;
              const auto entryIdBytes = entryId.toUtf8();
              QString submitError;
              const auto submittedJson = takeWorkerFfiString(
                  submitFn(runtimeDirBytes.constData(), entryIdBytes.constData()),
                  freeString, &submitError);
              const auto submittedValue =
                  submitError.isEmpty()
                      ? resultValueFromWorkerJson(submittedJson, &submitError)
                      : QJsonObject();
              if (!submittedValue.isEmpty()) {
                ++deliveredCount;
              }
            }
          }

          if (guard.isNull()) {
            return;
          }
          QMetaObject::invokeMethod(
              guard.data(),
              [guard, deliveredCount]() {
                if (guard.isNull()) {
                  return;
                }
                guard->setJoinRequestOutboxInFlight(false);
                if (deliveredCount > 0 && !guard->hasRuntimeWorkspace()) {
                  guard->setSyncStatus(QStringLiteral("access request delivered"));
                }
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runJoinResponseInboxRefresh(bool userInitiated) {
    const QPointer<ChaftController> guard(this);
    const auto listFn = m_listJoinResponseInboxJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto localDeviceId = m_deviceId.trimmed();
    auto *thread = QThread::create([guard, listFn, freeString, runtimeDir,
                                    localDeviceId, userInitiated]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      QString error;
      const auto listJson = takeWorkerFfiString(
          listFn(runtimeDirBytes.constData(), kMaxJoinResponseInboxEntries),
          freeString, &error);
      const auto listValue =
          error.isEmpty() ? resultValueFromWorkerJson(listJson, &error)
                          : QJsonObject();

      QString firstInviteText;
      QString firstInviteEntryId;
      QJsonArray responseUpdates;
      int responseCount = 0;
      if (!listValue.isEmpty()) {
        const auto entries = listValue.value(QStringLiteral("entries")).toArray();
        for (const auto &entryValue : entries) {
          const auto entry = entryValue.toObject();
          const auto entryId =
              entry.value(QStringLiteral("entryId")).toString().trimmed();
          const auto entryWorkspaceId =
              entry.value(QStringLiteral("workspaceId")).toString().trimmed();
          const auto responseText =
              entry.value(QStringLiteral("responseText")).toString().trimmed();
          if (responseText.isEmpty()) {
            continue;
          }
          QJsonParseError parseError;
          const auto responseDocument =
              QJsonDocument::fromJson(responseText.toUtf8(), &parseError);
          if (parseError.error != QJsonParseError::NoError ||
              !responseDocument.isObject()) {
            continue;
          }
          const auto response = responseDocument.object();
          const auto kind =
              response.value(QStringLiteral("kind")).toString().trimmed();
          const auto requestId =
              response.value(QStringLiteral("requestId")).toString().trimmed();
          const auto responseWorkspaceId =
              response.value(QStringLiteral("workspaceId"))
                  .toString()
                  .trimmed();
          const auto workspaceId =
              responseWorkspaceId.isEmpty() ? entryWorkspaceId
                                            : responseWorkspaceId;
          const auto createdAt =
              response.value(QStringLiteral("createdAt")).toString().trimmed();
          if (kind == QStringLiteral("chaft.workspace-invite.v1") ||
              kind == QStringLiteral("chaft.workspace-invite-response.v1")) {
            if (kind ==
                    QStringLiteral("chaft.workspace-invite-response.v1") &&
                response.value(QStringLiteral("inviteeDeviceId"))
                        .toString()
                        .trimmed() != localDeviceId) {
              continue;
            }
            ++responseCount;
            if (!requestId.isEmpty()) {
              QJsonObject update;
              update.insert(QStringLiteral("requestId"), requestId);
              update.insert(QStringLiteral("workspaceId"), workspaceId);
              update.insert(QStringLiteral("status"),
                            QStringLiteral("approved"));
              update.insert(QStringLiteral("resolvedAt"), createdAt);
              responseUpdates.append(update);
            }
            if (firstInviteText.isEmpty()) {
              firstInviteText = responseText;
              firstInviteEntryId = entryId;
            }
            continue;
          }
          if (kind != QStringLiteral("chaft.workspace-join-response.v1") ||
              requestId.isEmpty()) {
            continue;
          }
          const auto resolution =
              response.value(QStringLiteral("resolution")).toString().trimmed();
          QString status;
          if (resolution == QStringLiteral("approved")) {
            status = QStringLiteral("approved");
          } else if (resolution == QStringLiteral("declined")) {
            status = QStringLiteral("declined");
          } else if (resolution == QStringLiteral("revoked")) {
            status = QStringLiteral("closed");
          } else {
            continue;
          }
          ++responseCount;
          QJsonObject update;
          update.insert(QStringLiteral("entryId"), entryId);
          update.insert(QStringLiteral("requestId"), requestId);
          update.insert(QStringLiteral("workspaceId"), workspaceId);
          update.insert(QStringLiteral("status"), status);
          update.insert(QStringLiteral("message"),
                        response.value(QStringLiteral("message"))
                            .toString()
                            .trimmed());
          update.insert(QStringLiteral("resolvedAt"), createdAt);
          responseUpdates.append(update);
        }
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, listValue, error, userInitiated, firstInviteText,
           firstInviteEntryId, responseUpdates, responseCount]() {
            if (guard.isNull()) {
              return;
            }
            guard->setJoinResponseInboxInFlight(false);
            if (listValue.isEmpty()) {
              if (userInitiated) {
                guard->setSyncStatus(error);
              }
              return;
            }
            int appliedResponseCount = 0;
            QString lastAppliedStatus;
            for (const auto &updateValue : responseUpdates) {
              const auto update = updateValue.toObject();
              const auto status =
                  update.value(QStringLiteral("status")).toString().trimmed();
              if (status.isEmpty()) {
                continue;
              }
              if (guard->applyPendingJoinRequestResponse(
                      update.value(QStringLiteral("requestId"))
                          .toString()
                          .trimmed(),
                      update.value(QStringLiteral("workspaceId"))
                          .toString()
                          .trimmed(),
                      status,
                      update.value(QStringLiteral("message"))
                          .toString()
                          .trimmed(),
                      update.value(QStringLiteral("resolvedAt"))
                          .toString()
                          .trimmed())) {
                ++appliedResponseCount;
                lastAppliedStatus = status;
                const auto entryId =
                    update.value(QStringLiteral("entryId"))
                        .toString()
                        .trimmed();
                if (!entryId.isEmpty()) {
                  guard->acknowledgeJoinResponseInboxEntry(entryId, false);
                }
              }
            }
            if (!firstInviteText.isEmpty()) {
              if (!accessApprovalMatchesCurrentHandoff(
                      guard->m_keyTransferJson, firstInviteText)) {
                if (userInitiated) {
                  guard->setSyncStatus(QStringLiteral(
                      "access approval received; finish current handoff first"));
                }
                return;
              }
              guard->setKeyTransferJsonFromJoinResponseInbox(firstInviteText,
                                                             firstInviteEntryId);
              guard->setSyncStatus(QStringLiteral("access approval received"));
              return;
            }
            if (appliedResponseCount > 0) {
              if (lastAppliedStatus == QStringLiteral("declined")) {
                guard->setSyncStatus(QStringLiteral("access request declined"));
              } else if (lastAppliedStatus == QStringLiteral("closed")) {
                guard->setSyncStatus(QStringLiteral("access request closed"));
              } else {
                guard->setSyncStatus(QStringLiteral("access request updated"));
              }
              return;
            }
            if (userInitiated && responseCount == 0) {
              guard->setSyncStatus(QStringLiteral("no access approvals found"));
            }
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runJoinResponseOutboxDrain() {
    const QPointer<ChaftController> guard(this);
    const auto listFn = m_listDueJoinResponseOutboxJson;
    const auto submitFn = m_submitJoinResponseOutboxEntryDirectJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    auto *thread = QThread::create([guard, listFn, submitFn, freeString,
                                    runtimeDir]() {
      const auto runtimeDirBytes = runtimeDir.toUtf8();
      QString error;
      const auto listJson = takeWorkerFfiString(
          listFn(runtimeDirBytes.constData(), kMaxJoinResponseOutboxEntries),
          freeString, &error);
      const auto listValue =
          error.isEmpty() ? resultValueFromWorkerJson(listJson, &error)
                          : QJsonObject();
      int deliveredCount = 0;
      int attemptedCount = 0;
      if (!listValue.isEmpty()) {
        const auto entries = listValue.value(QStringLiteral("entries")).toArray();
        for (const auto &entryValue : entries) {
          if (attemptedCount >= kMaxJoinResponseOutboxDrainBatch) {
            break;
          }
          const auto entry = entryValue.toObject();
          const auto entryId =
              entry.value(QStringLiteral("entryId")).toString().trimmed();
          const auto peerEndpoint =
              entry.value(QStringLiteral("peerEndpoint")).toString().trimmed();
          if (entryId.isEmpty() || peerEndpoint.isEmpty()) {
            continue;
          }
          ++attemptedCount;
          const auto entryIdBytes = entryId.toUtf8();
          QString submitError;
          const auto submittedJson = takeWorkerFfiString(
              submitFn(runtimeDirBytes.constData(), entryIdBytes.constData()),
              freeString, &submitError);
          const auto submittedValue =
              submitError.isEmpty()
                  ? resultValueFromWorkerJson(submittedJson, &submitError)
                  : QJsonObject();
          if (!submittedValue.isEmpty()) {
            ++deliveredCount;
          }
        }
      }

      if (guard.isNull()) {
        return;
      }
      QMetaObject::invokeMethod(
          guard.data(),
          [guard, deliveredCount]() {
            if (guard.isNull()) {
              return;
            }
            guard->setJoinResponseOutboxInFlight(false);
            if (deliveredCount > 0) {
              guard->setSyncStatus(QStringLiteral("access response delivered"));
            }
          },
          Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceJoinRequestResolve(const QString &requestId,
                                      const QString &resolution,
                                      const QString &responseDeliveryPeerEndpoint,
                                      quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto resolveFn = m_resolveWorkspaceJoinRequestJson;
    const auto queueResponseOutboxFn = m_queueJoinResponseOutboxJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto responderDeviceId = m_deviceId.trimmed();
    const auto responderDisplayName =
        profileDisplayNameForDevice(m_workspaceSnapshot, responderDeviceId);
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, resolveFn, snapshotFn, snapshotLatestFn, freeString,
         runtimeDir, identityFile, workspaceId, requestId, resolution,
         responseDeliveryPeerEndpoint, responderDeviceId, responderDisplayName,
         queueResponseOutboxFn, generation, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto requestIdBytes = requestId.toUtf8();
          const auto resolutionBytes = resolution.toUtf8();
          char *raw = resolveFn(
              runtimeDirBytes.constData(),
              identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
              workspaceIdBytes.constData(), requestIdBytes.constData(),
              resolutionBytes.constData());

          QString error;
          const auto json = takeFfiString(raw, freeString, &error);
          const auto value =
              error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();

          bool queuedResponseDelivery = false;
          QString responseDeliveryError;
          if (!value.isEmpty() && queueResponseOutboxFn != nullptr &&
              !responseDeliveryPeerEndpoint.isEmpty() &&
              (resolution == QStringLiteral("declined") ||
               resolution == QStringLiteral("revoked"))) {
            const auto responseBytes = workspaceJoinResponseJson(
                workspaceId, requestId, resolution, responderDeviceId,
                responderDisplayName);
            if (responseBytes.size() > kMaxKeyTransferJsonBytes) {
              responseDeliveryError =
                  QStringLiteral("access response is too large");
            } else {
              const auto responseDeliveryPeerEndpointBytes =
                  responseDeliveryPeerEndpoint.toUtf8();
              QString queueError;
              const auto queuedJson = takeWorkerFfiString(
                  queueResponseOutboxFn(runtimeDirBytes.constData(),
                                        responseDeliveryPeerEndpointBytes
                                            .constData(),
                                        workspaceIdBytes.constData(),
                                        responseBytes.constData()),
                  freeString, &queueError);
              const auto queuedValue =
                  queueError.isEmpty()
                      ? resultValueFromWorkerJson(queuedJson, &queueError)
                      : QJsonObject();
              queuedResponseDelivery = !queuedValue.isEmpty();
              if (!queuedResponseDelivery) {
                responseDeliveryError = queueError;
              }
            }
          }
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
               queuedResponseDelivery, responseDeliveryError, generation]() {
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
                      "access request updated after switching workspaces"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                if (queuedResponseDelivery) {
                  guard->setSyncStatus(
                      QStringLiteral("access request updated; response queued"));
                  guard->queueJoinResponseOutboxDrain();
                } else if (!responseDeliveryError.isEmpty()) {
                  guard->setSyncStatus(QStringLiteral(
                      "access request updated; response not queued"));
                } else {
                  guard->setSyncStatus(
                      QStringLiteral("access request updated"));
                }
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceInviteResolve(const QString &inviteId,
                                 const QString &resolution,
                                 quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto resolveFn = m_resolveWorkspaceInviteJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, resolveFn, snapshotFn, snapshotLatestFn, freeString,
         runtimeDir, identityFile, workspaceId, inviteId, resolution,
         generation, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto inviteIdBytes = inviteId.toUtf8();
          const auto resolutionBytes = resolution.toUtf8();
          char *raw = resolveFn(
              runtimeDirBytes.constData(),
              identityFileBytes.isEmpty() ? nullptr : identityFileBytes.constData(),
              workspaceIdBytes.constData(), inviteIdBytes.constData(),
              resolutionBytes.constData());

          QString error;
          const auto json = takeFfiString(raw, freeString, &error);
          const auto value =
              error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
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
                      QStringLiteral("invite updated after switching workspaces"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setSyncStatus(QStringLiteral("invite updated"));
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runMemberRoleUpdate(const QString &deviceId, const QString &role,
                           quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto updateFn = m_updateMemberRoleJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread =
        QThread::create([guard, updateFn, snapshotFn, snapshotLatestFn,
                         freeString, runtimeDir, identityFile, workspaceId,
                         deviceId, role, generation, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto deviceIdBytes = deviceId.toUtf8();
          const auto roleBytes = role.toUtf8();
          char *raw = updateFn(
              runtimeDirBytes.constData(),
              identityFileBytes.isEmpty() ? nullptr
                                          : identityFileBytes.constData(),
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
                      QStringLiteral("role updated after switching workspaces"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setSyncStatus(QStringLiteral("role updated"));
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceInvitePackage(const QString &deviceId, const QString &role,
                                 const QString &peerEndpoint,
                                 const QString &inviteeDisplayName,
                                 const QString &expiresAt,
                                 const QString &approvalPolicy,
                                 const QString &requestId,
                                 const QString &responseDeliveryPeerEndpoint,
                                 quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto inviteFn = m_inviteMemberJson;
    const auto recordInviteFn = m_recordWorkspaceInviteJson;
    const auto exportFn = m_exportWorkspaceKeyJson;
    const auto queueResponseOutboxFn = m_queueJoinResponseOutboxJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto workspaceName =
        m_workspaceSnapshot.value(QStringLiteral("name")).toString();
    const auto inviterDeviceId = m_deviceId.trimmed();
    const auto inviterDisplayName =
        profileDisplayNameForDevice(m_workspaceSnapshot, inviterDeviceId);
    const auto inviteId = generatedInviteId();
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, inviteFn, exportFn, snapshotFn, snapshotLatestFn, freeString,
         runtimeDir, identityFile, workspaceId, workspaceName, deviceId, role,
         peerEndpoint, inviteeDisplayName, expiresAt, approvalPolicy, requestId,
         responseDeliveryPeerEndpoint, inviterDeviceId, inviterDisplayName,
         inviteId, recordInviteFn, queueResponseOutboxFn, generation,
         timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto deviceIdBytes = deviceId.toUtf8();
          const auto roleBytes = role.toUtf8();
          const auto inviteIdBytes = inviteId.toUtf8();
          const auto inviteeDisplayNameBytes = inviteeDisplayName.toUtf8();
          const auto expiresAtBytes = expiresAt.toUtf8();
          const auto approvalPolicyBytes = approvalPolicy.toUtf8();
          const auto requestIdBytes = requestId.toUtf8();
          const auto syncExpectationBytes =
              inviteSyncExpectation(peerEndpoint, approvalPolicy).toUtf8();

          QString error;
          const auto inviteJson = takeWorkerFfiString(
              inviteFn(runtimeDirBytes.constData(),
                       identityFileBytes.isEmpty()
                           ? nullptr
                           : identityFileBytes.constData(),
                       workspaceIdBytes.constData(), deviceIdBytes.constData(),
                       roleBytes.constData()),
              freeString, &error);
          const auto inviteValue =
              error.isEmpty() ? resultValueFromWorkerJson(inviteJson, &error)
                              : QJsonObject();

          if (!inviteValue.isEmpty()) {
            const auto recordInviteJson = takeWorkerFfiString(
                recordInviteFn(runtimeDirBytes.constData(),
                               identityFileBytes.isEmpty()
                                   ? nullptr
                                   : identityFileBytes.constData(),
                               workspaceIdBytes.constData(),
                               inviteIdBytes.constData(), deviceIdBytes.constData(),
                               inviteeDisplayNameBytes.constData(),
                               roleBytes.constData(),
                               requestIdBytes.isEmpty()
                                   ? nullptr
                                   : requestIdBytes.constData(),
                               expiresAtBytes.constData(),
                               approvalPolicyBytes.constData(),
                               syncExpectationBytes.constData()),
                freeString, &error);
            if (error.isEmpty()) {
              resultValueFromWorkerJson(recordInviteJson, &error);
            }
          }

          QJsonObject workspaceKey;
          if (!inviteValue.isEmpty()) {
            const auto exportJson = takeWorkerFfiString(
                exportFn(runtimeDirBytes.constData(),
                         identityFileBytes.isEmpty()
                             ? nullptr
                             : identityFileBytes.constData(),
                         workspaceIdBytes.constData()),
                freeString, &error);
            workspaceKey = error.isEmpty()
                               ? resultValueFromWorkerJson(exportJson, &error)
                               : QJsonObject();
            if (error.isEmpty() && !exportedWorkspaceKeyLooksValid(workspaceKey)) {
              error = QStringLiteral("workspace access export required");
            }
          }

          QString packageJson;
          bool queuedResponseDelivery = false;
          if (error.isEmpty()) {
            const auto packageBytes = workspaceInvitePackageJson(
                workspaceId, workspaceName, inviteId, requestId, deviceId, inviteeDisplayName,
                role, peerEndpoint, inviterDeviceId, inviterDisplayName,
                expiresAt, approvalPolicy,
                workspaceKey);
            if (packageBytes.size() > kMaxKeyTransferJsonBytes) {
              error = QStringLiteral("invite package is too large");
            } else {
              packageJson = QString::fromUtf8(packageBytes);
            }
          }
          if (error.isEmpty() && queueResponseOutboxFn != nullptr &&
              !requestId.isEmpty() && !responseDeliveryPeerEndpoint.isEmpty()) {
            const auto responseDeliveryPeerEndpointBytes =
                responseDeliveryPeerEndpoint.toUtf8();
            const auto packageJsonBytes = packageJson.toUtf8();
            QString queueError;
            const auto queuedJson = takeWorkerFfiString(
                queueResponseOutboxFn(runtimeDirBytes.constData(),
                                      responseDeliveryPeerEndpointBytes.constData(),
                                      workspaceIdBytes.constData(),
                                      packageJsonBytes.constData()),
                freeString, &queueError);
            const auto queuedValue =
                queueError.isEmpty()
                    ? resultValueFromWorkerJson(queuedJson, &queueError)
                    : QJsonObject();
            queuedResponseDelivery = !queuedValue.isEmpty();
          }

          QJsonObject snapshotValue;
          QString snapshotError;
          if (error.isEmpty()) {
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
              [guard, packageJson, error, snapshotValue, snapshotError,
               workspaceId, generation, queuedResponseDelivery]() {
                if (guard.isNull()) {
                  return;
                }
                guard->setKeyTransferInFlight(false);
                if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
                  guard->queueRuntimeSnapshotRefreshIfCurrent(error.isEmpty(),
                                                              workspaceId);
                  return;
                }
                if (!error.isEmpty()) {
                  guard->setSyncStatus(error);
                  return;
                }
                if (snapshotValue.isEmpty()) {
                  guard->setSyncStatus(snapshotError);
                  return;
                }
                if (guard->m_workspaceId != workspaceId) {
                  guard->setSyncStatus(
                      QStringLiteral("invite created after switching workspaces"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setKeyTransferJson(packageJson);
                guard->setSyncStatus(
                    queuedResponseDelivery
                        ? QStringLiteral("invite ready; delivery queued")
                        : QStringLiteral("invite ready to share"));
                if (queuedResponseDelivery) {
                  guard->queueJoinResponseOutboxDrain();
                }
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runClaimableWorkspaceInvite(const QString &displayName,
                                   const QString &role,
                                   const QString &peerEndpoint,
                                   const QString &expiresAt,
                                   quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto createFn = m_createWorkspaceInviteJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    const auto syncExpectation = peerEndpoint.isEmpty()
                                     ? QStringLiteral("history_after_claim")
                                     : QStringLiteral("history_from_inviter");
    auto *thread = QThread::create(
        [guard, createFn, snapshotFn, snapshotLatestFn, freeString, runtimeDir,
         identityFile, workspaceId, displayName, role, peerEndpoint, expiresAt,
         syncExpectation, generation, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto displayNameBytes = displayName.toUtf8();
          const auto roleBytes = role.toUtf8();
          const auto expiresAtBytes = expiresAt.toUtf8();
          const auto peerEndpointBytes = peerEndpoint.toUtf8();
          const auto syncExpectationBytes = syncExpectation.toUtf8();

          QString error;
          const auto json = takeWorkerFfiString(
              createFn(runtimeDirBytes.constData(),
                       identityFileBytes.isEmpty()
                           ? nullptr
                           : identityFileBytes.constData(),
                       workspaceIdBytes.constData(), displayNameBytes.constData(),
                       roleBytes.constData(), expiresAtBytes.constData(),
                       peerEndpointBytes.constData(),
                       syncExpectationBytes.constData()),
              freeString, &error);
          const auto value =
              error.isEmpty() ? resultValueFromWorkerJson(json, &error)
                              : QJsonObject();
          const auto artifact = value.value(QStringLiteral("artifact")).toObject();
          QString artifactJson;
          if (!artifact.isEmpty()) {
            artifactJson = QString::fromUtf8(
                QJsonDocument(artifact).toJson(QJsonDocument::Indented));
          }

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
              [guard, value, artifactJson, error, snapshotValue, snapshotError,
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
                if (value.isEmpty() || artifactJson.isEmpty()) {
                  guard->setSyncStatus(error.isEmpty()
                                           ? QStringLiteral("secure invite unavailable")
                                           : error);
                  return;
                }
                if (snapshotValue.isEmpty()) {
                  guard->setSyncStatus(snapshotError);
                  return;
                }
                if (guard->m_workspaceId != workspaceId) {
                  guard->setSyncStatus(QStringLiteral(
                      "secure invite created after switching workspaces"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }
                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                const auto artifactRemembered =
                    guard->rememberWorkspaceInviteArtifact(artifactJson);
                guard->setKeyTransferJson(artifactJson);
                guard->setSyncStatus(
                    artifactRemembered
                        ? QStringLiteral("secure invite ready to share")
                        : QStringLiteral(
                              "secure invite ready; copy or save it before creating another"));
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceApprovalInvitePackage(const QString &deviceId,
                                         const QString &role,
                                         const QString &peerEndpoint,
                                         const QString &inviteeDisplayName,
                                         const QString &expiresAt,
                                         quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto recordInviteFn = m_recordWorkspaceInviteJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto workspaceName =
        m_workspaceSnapshot.value(QStringLiteral("name")).toString();
    const auto inviterDeviceId = m_deviceId.trimmed();
    const auto inviterDisplayName =
        profileDisplayNameForDevice(m_workspaceSnapshot, inviterDeviceId);
    const auto inviteId = generatedInviteId();
    const auto approvalPolicy = QStringLiteral("admin_required");
    const auto syncExpectation = inviteSyncExpectation(peerEndpoint, approvalPolicy);
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread = QThread::create(
        [guard, snapshotFn, snapshotLatestFn, freeString, runtimeDir,
         identityFile, workspaceId, workspaceName, deviceId, role, peerEndpoint,
         inviteeDisplayName, expiresAt, inviterDeviceId, inviterDisplayName,
         inviteId, recordInviteFn, approvalPolicy, syncExpectation, generation,
         timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto deviceIdBytes = deviceId.toUtf8();
          const auto roleBytes = role.toUtf8();
          const auto inviteIdBytes = inviteId.toUtf8();
          const auto inviteeDisplayNameBytes = inviteeDisplayName.toUtf8();
          const auto expiresAtBytes = expiresAt.toUtf8();
          const auto approvalPolicyBytes = approvalPolicy.toUtf8();
          const auto syncExpectationBytes = syncExpectation.toUtf8();

          QString error;
          const auto recordInviteJson = takeWorkerFfiString(
              recordInviteFn(runtimeDirBytes.constData(),
                             identityFileBytes.isEmpty()
                                 ? nullptr
                                 : identityFileBytes.constData(),
                             workspaceIdBytes.constData(),
                             inviteIdBytes.constData(), deviceIdBytes.constData(),
                             inviteeDisplayNameBytes.constData(),
                             roleBytes.constData(), nullptr,
                             expiresAtBytes.constData(),
                             approvalPolicyBytes.constData(),
                             syncExpectationBytes.constData()),
              freeString, &error);
          if (error.isEmpty()) {
            resultValueFromWorkerJson(recordInviteJson, &error);
          }

          QString packageJson;
          if (error.isEmpty()) {
            const auto packageBytes = workspaceInvitePackageJson(
                workspaceId, workspaceName, inviteId, QString(), deviceId,
                inviteeDisplayName, role, peerEndpoint, inviterDeviceId,
                inviterDisplayName, expiresAt, approvalPolicy, QJsonObject());
            if (packageBytes.size() > kMaxKeyTransferJsonBytes) {
              error = QStringLiteral("invite package is too large");
            } else {
              packageJson = QString::fromUtf8(packageBytes);
            }
          }

          QJsonObject snapshotValue;
          QString snapshotError;
          if (error.isEmpty()) {
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
              [guard, packageJson, error, snapshotValue, snapshotError,
               workspaceId, generation]() {
                if (guard.isNull()) {
                  return;
                }
                guard->setKeyTransferInFlight(false);
                if (generation < guard->m_lastAppliedRuntimeWriteGeneration) {
                  guard->queueRuntimeSnapshotRefreshIfCurrent(error.isEmpty(),
                                                              workspaceId);
                  return;
                }
                if (!error.isEmpty()) {
                  guard->setSyncStatus(error);
                  return;
                }
                if (snapshotValue.isEmpty()) {
                  guard->setSyncStatus(snapshotError);
                  return;
                }
                if (guard->m_workspaceId != workspaceId) {
                  guard->setSyncStatus(
                      QStringLiteral("approval invite created after switching workspaces"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setKeyTransferJson(packageJson);
                guard->setSyncStatus(QStringLiteral("approval invite ready to share"));
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
  }

  void runWorkspaceAccessPolicyUpdate(const QString &accessPolicy,
                                      quint64 generation) {
    const QPointer<ChaftController> guard(this);
    const auto updateFn = m_updateWorkspaceAccessPolicyJson;
    const auto snapshotFn = m_runtimeSnapshotJson;
    const auto snapshotLatestFn = m_runtimeSnapshotLatestJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    const auto identityFile = m_identityFile;
    const auto workspaceId = m_workspaceId;
    const auto timelineLimit = configuredTimelineLimit();
    auto *thread =
        QThread::create([guard, updateFn, snapshotFn, snapshotLatestFn,
                         freeString, runtimeDir, identityFile, workspaceId,
                         accessPolicy, generation, timelineLimit]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto identityFileBytes = identityFile.toUtf8();
          const auto workspaceIdBytes = workspaceId.toUtf8();
          const auto accessPolicyBytes = accessPolicy.toUtf8();
          char *raw = updateFn(
              runtimeDirBytes.constData(),
              identityFileBytes.isEmpty() ? nullptr
                                          : identityFileBytes.constData(),
              workspaceIdBytes.constData(), accessPolicyBytes.constData());

          QString error;
          const auto json = takeFfiString(raw, freeString, &error);
          const auto value =
              error.isEmpty() ? resultValueFromJson(json, &error) : QJsonObject();
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
                  guard->setSyncStatus(QStringLiteral(
                      "workspace access refreshed after switching workspaces"));
                  guard->m_lastAppliedRuntimeWriteGeneration = generation;
                  return;
                }

                guard->applyRuntimeSnapshot(snapshotValue, false);
                guard->m_lastAppliedRuntimeWriteGeneration = generation;
                guard->setSyncStatus(QStringLiteral("workspace access refreshed"));
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
          successStatus = QStringLiteral("workspace member removed");
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
                QStringLiteral("workspace member removed and workspace access refreshed");
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
                  "room access granted after switching workspaces"));
              guard->m_lastAppliedRuntimeWriteGeneration = generation;
              return;
            }

            guard->applyRuntimeSnapshot(snapshotValue, false);
            guard->m_lastAppliedRuntimeWriteGeneration = generation;
            guard->setSyncStatus(QStringLiteral("room access granted"));
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
          successStatus = QStringLiteral("room access removed");
        } else if (!shouldFallbackFromOpenMlsRemovalError(openMlsError)) {
          error = openMlsError;
        }
      }

      if (value.isEmpty() && error.isEmpty()) {
        if (rotationFn == nullptr) {
          error = openMlsError.isEmpty()
                      ? QStringLiteral("room access removal unavailable")
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
                QStringLiteral("room access removed and refreshed");
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
                  QStringLiteral("room access removed after workspace "
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
                      QStringLiteral("message sent after switching workspaces"));
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
                  QStringLiteral("message edited after switching workspaces"));
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
                  QStringLiteral("message deleted after switching workspaces"));
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
                  QStringLiteral("reaction added after switching workspaces"));
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
                  QStringLiteral("reaction removed after switching workspaces"));
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
                      QStringLiteral("attachment sent after switching workspaces"));
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
                  "history loaded after switching workspaces"));
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
                  "room history loaded after switching workspaces"));
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (value.value(QStringLiteral("timelineChannelId")).toString() !=
                channelId) {
              guard->setSyncStatus(
                  QStringLiteral("room history page was stale"));
              return;
            }

            guard->m_workspaceSnapshot =
                guard->snapshotWithPreservedResolvedChannels(value);
            emit guard->workspaceSnapshotChanged();
            const auto timelineCount =
                value.value(QStringLiteral("timeline")).toArray().size();
            guard->setSyncStatus(QStringLiteral("loaded %1 message(s)")
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
                  "older room history loaded after switching workspaces"));
              return;
            }
            if (value.isEmpty()) {
              guard->setSyncStatus(error);
              return;
            }
            if (value.value(QStringLiteral("timelineChannelId")).toString() !=
                channelId) {
              guard->setSyncStatus(
                  QStringLiteral("room history page was stale"));
              return;
            }

            const auto timelineCount =
                value.value(QStringLiteral("timeline")).toArray().size();
            guard->prependTimelineWindow(value.toVariantMap());
            guard->setSyncStatus(
                QStringLiteral("loaded %1 older message(s)")
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

  void setJoinRequestSubmitInFlight(bool joinRequestSubmitInFlight) {
    if (m_joinRequestSubmitInFlight == joinRequestSubmitInFlight) {
      return;
    }
    m_joinRequestSubmitInFlight = joinRequestSubmitInFlight;
    emit joinRequestSubmitInFlightChanged();
  }

  void setJoinRequestOutboxInFlight(bool joinRequestOutboxInFlight) {
    m_joinRequestOutboxInFlight = joinRequestOutboxInFlight;
  }

  void setJoinRequestInboxInFlight(bool joinRequestInboxInFlight) {
    if (m_joinRequestInboxInFlight == joinRequestInboxInFlight) {
      return;
    }
    m_joinRequestInboxInFlight = joinRequestInboxInFlight;
    emit joinRequestInboxInFlightChanged();
  }

  void setJoinResponseOutboxInFlight(bool joinResponseOutboxInFlight) {
    m_joinResponseOutboxInFlight = joinResponseOutboxInFlight;
  }

  void setJoinResponseInboxInFlight(bool joinResponseInboxInFlight) {
    m_joinResponseInboxInFlight = joinResponseInboxInFlight;
  }

  void setAccessEnvelopePullInFlight(bool accessEnvelopePullInFlight) {
    if (m_accessEnvelopePullInFlight == accessEnvelopePullInFlight) {
      return;
    }
    m_accessEnvelopePullInFlight = accessEnvelopePullInFlight;
    emit accessEnvelopePullInFlightChanged();
  }

  bool acknowledgeJoinResponseInboxEntry(const QString &entryId,
                                         bool clearCurrentHandoffSource) {
    const auto normalizedEntryId = entryId.trimmed();
    if (normalizedEntryId.isEmpty() || !m_ffiReady ||
        m_ackJoinResponseInboxEntryJson == nullptr || m_freeString == nullptr ||
        m_runtimeDir.trimmed().isEmpty()) {
      return false;
    }

    const QPointer<ChaftController> guard(this);
    const auto ackFn = m_ackJoinResponseInboxEntryJson;
    const auto freeString = m_freeString;
    const auto runtimeDir = m_runtimeDir;
    auto *thread = QThread::create(
        [guard, ackFn, freeString, runtimeDir, normalizedEntryId,
         clearCurrentHandoffSource]() {
          const auto runtimeDirBytes = runtimeDir.toUtf8();
          const auto entryIdBytes = normalizedEntryId.toUtf8();
          QString error;
          const auto ackJson = takeWorkerFfiString(
              ackFn(runtimeDirBytes.constData(), entryIdBytes.constData()),
              freeString, &error);
          const auto ackValue =
              error.isEmpty() ? resultValueFromWorkerJson(ackJson, &error)
                              : QJsonObject();
          if (guard.isNull() || ackValue.isEmpty()) {
            return;
          }
          QMetaObject::invokeMethod(
              guard.data(),
              [guard, normalizedEntryId, clearCurrentHandoffSource]() {
                if (guard.isNull()) {
                  return;
                }
                if (clearCurrentHandoffSource &&
                    guard->m_keyTransferJoinResponseInboxEntryId ==
                        normalizedEntryId) {
                  guard->m_keyTransferJoinResponseInboxEntryId.clear();
                  emit guard->keyTransferJsonChanged();
                }
              },
              Qt::QueuedConnection);
        });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
    return true;
  }

  void setKeyTransferJsonFromJoinResponseInbox(const QString &keyTransferJson,
                                               const QString &entryId) {
    const auto normalizedEntryId = entryId.trimmed();
    const auto jsonChanged = m_keyTransferJson != keyTransferJson;
    const auto sourceChanged =
        m_keyTransferJoinResponseInboxEntryId != normalizedEntryId;
    m_keyTransferJson = keyTransferJson;
    m_keyTransferJoinResponseInboxEntryId = normalizedEntryId;
    if (jsonChanged || sourceChanged) {
      emit keyTransferJsonChanged();
    }
  }

  void setKeyTransferJson(const QString &keyTransferJson) {
    const auto sourceChanged = !m_keyTransferJoinResponseInboxEntryId.isEmpty();
    m_keyTransferJoinResponseInboxEntryId.clear();
    if (m_keyTransferJson == keyTransferJson && !sourceChanged) {
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

  void setLastRecoveryImportedChannelCount(int count) {
    const auto boundedCount = count < 0 ? 0 : count;
    if (m_lastRecoveryImportedChannelCount == boundedCount) {
      return;
    }
    m_lastRecoveryImportedChannelCount = boundedCount;
    emit lastRecoveryImportedChannelCountChanged();
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

  void setLastCreatedChannelId(const QString &channelId) {
    if (m_lastCreatedChannelId == channelId) {
      return;
    }
    m_lastCreatedChannelId = channelId;
    emit lastCreatedChannelChanged();
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
  RuntimeCreateWorkspaceWithAccessPolicyResultJsonFn
      m_createWorkspaceWithAccessPolicyJson = nullptr;
  RuntimeCreateChannelResultJsonFn m_createChannelJson = nullptr;
  RuntimeCreateDirectMessageChannelResultJsonFn
      m_createDirectMessageChannelJson = nullptr;
  RuntimeUpdateChannelDetailsResultJsonFn m_updateChannelDetailsJson = nullptr;
  RuntimeUpdateChannelArchiveResultJsonFn m_updateChannelArchiveJson = nullptr;
  RuntimeUpdateDeviceProfileResultJsonFn m_updateDeviceProfileJson = nullptr;
  RuntimeUpdateLocalPersonProfileResultJsonFn m_updateLocalPersonProfileJson =
      nullptr;
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
  RuntimeCreateWorkspaceInviteResultJsonFn m_createWorkspaceInviteJson = nullptr;
  RuntimePrepareWorkspaceInviteClaimResultJsonFn
      m_prepareWorkspaceInviteClaimJson = nullptr;
  RuntimeWorkspaceInviteEnvelopeResultJsonFn m_claimWorkspaceInviteJson = nullptr;
  RuntimeWorkspaceInviteEnvelopeResultJsonFn
      m_importWorkspaceInviteResponseJson = nullptr;
  RuntimeRecordWorkspaceInviteResultJsonFn
      m_recordWorkspaceInviteJson = nullptr;
  RuntimeResolveWorkspaceInviteResultJsonFn
      m_resolveWorkspaceInviteJson = nullptr;
  RuntimeRecordWorkspaceJoinRequestResultJsonFn
      m_recordWorkspaceJoinRequestJson = nullptr;
  RuntimeRecordWorkspaceJoinRequestWithResponseRouteResultJsonFn
      m_recordWorkspaceJoinRequestWithResponseRouteJson = nullptr;
  RuntimeResolveWorkspaceJoinRequestResultJsonFn
      m_resolveWorkspaceJoinRequestJson = nullptr;
  RuntimeUpdateMemberRoleResultJsonFn m_updateMemberRoleJson = nullptr;
  RuntimeUpdateWorkspaceAccessPolicyResultJsonFn
      m_updateWorkspaceAccessPolicyJson = nullptr;
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
  RuntimeSubmitJoinRequestDirectResultJsonFn
      m_submitJoinRequestDirectJson = nullptr;
  RuntimePullJoinAccessDirectResultJsonFn m_pullJoinRequestsDirectJson =
      nullptr;
  RuntimePullJoinAccessDirectResultJsonFn m_pullJoinResponsesDirectJson =
      nullptr;
  RuntimeWorkspacePublishQueueResultJsonFn m_workspacePublishQueueJson =
      nullptr;
  RuntimeWorkspaceStorageHealthResultJsonFn m_workspaceStorageHealthJson =
      nullptr;
  RuntimeRepairWorkspaceStorageMetadataResultJsonFn
      m_repairWorkspaceStorageMetadataJson = nullptr;
  RuntimeStartDirectPeerResultJsonFn m_startDirectPeerJson = nullptr;
  RuntimeStartIrohPeerResultJsonFn m_startIrohPeerJson = nullptr;
  RuntimeListJoinRequestInboxResultJsonFn m_listJoinRequestInboxJson = nullptr;
  RuntimeAckJoinRequestInboxEntryResultJsonFn
      m_ackJoinRequestInboxEntryJson = nullptr;
  RuntimeQueueJoinRequestOutboxResultJsonFn m_queueJoinRequestOutboxJson =
      nullptr;
  RuntimeListJoinRequestOutboxResultJsonFn m_listJoinRequestOutboxJson =
      nullptr;
  RuntimeListJoinRequestOutboxResultJsonFn m_listDueJoinRequestOutboxJson =
      nullptr;
  RuntimeSubmitJoinRequestOutboxEntryDirectResultJsonFn
      m_submitJoinRequestOutboxEntryDirectJson = nullptr;
  RuntimeListJoinResponseInboxResultJsonFn m_listJoinResponseInboxJson =
      nullptr;
  RuntimeAckJoinResponseInboxEntryResultJsonFn
      m_ackJoinResponseInboxEntryJson = nullptr;
  RuntimeStageJoinResponseInboxResultJsonFn m_stageJoinResponseInboxJson = nullptr;
  RuntimeQueueJoinResponseOutboxResultJsonFn m_queueJoinResponseOutboxJson =
      nullptr;
  RuntimeListJoinResponseOutboxResultJsonFn m_listJoinResponseOutboxJson =
      nullptr;
  RuntimeListJoinResponseOutboxResultJsonFn m_listDueJoinResponseOutboxJson =
      nullptr;
  RuntimeSubmitJoinResponseOutboxEntryDirectResultJsonFn
      m_submitJoinResponseOutboxEntryDirectJson = nullptr;
  RuntimeStopDirectPeerResultJsonFn m_stopDirectPeerJson = nullptr;
  RuntimeSetIdentityPassphraseFn m_setIdentityPassphrase = nullptr;
  RuntimeClearIdentityPassphraseFn m_clearIdentityPassphrase = nullptr;
  FreeStringFn m_freeString = nullptr;
  std::unique_ptr<QSystemTrayIcon> m_notificationTrayIcon;
  bool m_ffiReady = false;
  bool m_syncInFlight = false;
  bool m_channelPageInFlight = false;
  bool m_memberPageInFlight = false;
  bool m_peerHostingInFlight = false;
  bool m_backgroundReachabilityFallbackPending = false;
  bool m_backgroundReachabilityRetryScheduled = false;
  bool m_backgroundReachabilityStoppedByUser = false;
  bool m_runtimeBackgroundServicesStarted = false;
  int m_backgroundReachabilityRetryAttempt = 0;
  bool m_joinRequestInboxInFlight = false;
  bool m_joinRequestOutboxInFlight = false;
  bool m_joinRequestSubmitInFlight = false;
  bool m_joinResponseInboxInFlight = false;
  bool m_joinResponseOutboxInFlight = false;
  bool m_accessEnvelopePullInFlight = false;
  bool m_keyTransferInFlight = false;
  int m_lastRecoveryImportedChannelCount = 0;
  bool m_autoBackupEnabled = false;
  bool m_inspectorPinned = false;
  bool m_reducedMotionEnabled = false;
  bool m_notificationsEnabled = true;
  bool m_notificationSoundEnabled = true;
  bool m_notificationPreviewEnabled = true;
  bool m_externalLinkConfirmationEnabled = true;
  QVariantMap m_mutedChannels;
  QVariantMap m_composerDrafts;
  QVariantMap m_pendingJoinRequests;
  QVariantMap m_workspaceInviteArtifacts;
  QVariantMap m_windowGeometry;
  QString m_themeId;
  QString m_themeMode = QStringLiteral("manual");
  QString m_darkThemeId;
  QString m_lightThemeId;
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
  QString m_lastCreatedChannelId;
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
  quint64 m_joinRequestInboxGeneration = 0;
  QString m_syncStatus;
  QString m_keyTransferJson;
  QString m_keyTransferJoinResponseInboxEntryId;
  qsizetype m_nextBackupPeerIndex = 0;
};

bool desktopSmokeFlagEnabled() {
  const auto value = qEnvironmentVariable("CHAFT_DESKTOP_SMOKE")
                         .trimmed()
                         .toLower();
  return value == QStringLiteral("1") || value == QStringLiteral("true") ||
         value == QStringLiteral("yes") || value == QStringLiteral("on");
}

bool desktopSmokeExpectNoWorkspace() {
  const auto value = qEnvironmentVariable("CHAFT_DESKTOP_SMOKE_EXPECT_NO_WORKSPACE")
                         .trimmed()
                         .toLower();
  return value == QStringLiteral("1") || value == QStringLiteral("true") ||
         value == QStringLiteral("yes") || value == QStringLiteral("on");
}

bool desktopSmokeExpectReachable() {
  return parseEnabledFlag(
      qEnvironmentVariable("CHAFT_DESKTOP_SMOKE_EXPECT_REACHABLE"));
}

QString desktopSmokeExpectedReachabilityRoute() {
  const auto route =
      qEnvironmentVariable("CHAFT_DESKTOP_SMOKE_EXPECT_ROUTE").trimmed().toLower();
  if (route == QStringLiteral("iroh") ||
      route == QStringLiteral("iroh-direct") ||
      route == QStringLiteral("iroh-relay") ||
      route == QStringLiteral("iroh-discovery") ||
      route == QStringLiteral("direct")) {
    return route;
  }
  return {};
}

bool desktopSmokeReachabilityMatches(ChaftController *controller,
                                     const QString &expectedRoute) {
  if (controller == nullptr || !controller->peerHosting()) {
    return false;
  }
  if (expectedRoute.isEmpty()) {
    return true;
  }
  const auto endpoint = controller->hostedPeerEndpoint().trimmed().toLower();
  const auto isIroh = endpoint.startsWith(QStringLiteral("iroh://"));
  if (expectedRoute == QStringLiteral("direct")) {
    return !isIroh;
  }
  if (!isIroh) {
    return false;
  }
  if (expectedRoute == QStringLiteral("iroh-direct")) {
    return endpoint.contains(QStringLiteral("?addr=")) &&
           !endpoint.contains(QStringLiteral("relay="));
  }
  if (expectedRoute == QStringLiteral("iroh-relay")) {
    return endpoint.contains(QStringLiteral("relay="));
  }
  if (expectedRoute == QStringLiteral("iroh-discovery")) {
    return !endpoint.contains(QLatin1Char('?'));
  }
  return true;
}

bool desktopSmokeQuickItemCaptureEnabled() {
  const auto value = qEnvironmentVariable("CHAFT_DESKTOP_SMOKE_QUICK_ITEM_CAPTURE")
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

int desktopSmokeScreenshotDelayMs() {
  const auto value =
      qEnvironmentVariable("CHAFT_DESKTOP_SMOKE_SCREENSHOT_DELAY_MS").trimmed();
  bool ok = false;
  const auto parsed = value.toInt(&ok);
  if (!ok) {
    return 250;
  }
  return qBound(0, parsed, 10000);
}

int desktopSmokeScreenshotCaptureTimeoutMs() {
  const auto value =
      qEnvironmentVariable("CHAFT_DESKTOP_SMOKE_CAPTURE_TIMEOUT_MS").trimmed();
  bool ok = false;
  const auto parsed = value.toInt(&ok);
  if (!ok) {
    return 10000;
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

bool desktopSmokeReadyForEmptyRuntime(ChaftController *controller,
                                      const QVariantMap &snapshot) {
  if (controller == nullptr) {
    return false;
  }
  if (!snapshot.value(QStringLiteral("workspaceId")).toString().isEmpty()) {
    return false;
  }
  const auto status = controller->syncStatus().trimmed().toLower();
  return !status.isEmpty() && !status.startsWith(QStringLiteral("loading"));
}

bool ensureDesktopSmokeScreenshotDirectory(const QString &path,
                                           QString *errorMessage) {
  const QFileInfo fileInfo(path);
  const auto outputDir = fileInfo.absoluteDir();
  if (outputDir.exists() || QDir().mkpath(outputDir.absolutePath())) {
    return true;
  }
  if (errorMessage != nullptr) {
    *errorMessage =
        QStringLiteral("failed to create screenshot directory: %1")
            .arg(outputDir.absolutePath());
  }
  return false;
}

QQuickItem *desktopSmokeRootQuickItem(QQmlApplicationEngine *engine) {
  if (engine == nullptr) {
    return nullptr;
  }
  const auto roots = engine->rootObjects();
  for (auto *root : roots) {
    if (auto *item = qobject_cast<QQuickItem *>(root)) {
      return item;
    }
    if (auto *window = qobject_cast<QQuickWindow *>(root)) {
      return window->contentItem();
    }
  }
  return nullptr;
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

  if (!ensureDesktopSmokeScreenshotDirectory(path, errorMessage)) {
    return false;
  }

  QString captureError;
  const auto writeFailure = [&path, errorMessage]() {
    if (errorMessage != nullptr) {
      *errorMessage =
          QStringLiteral("failed to write desktop screenshot: %1").arg(path);
    }
    return false;
  };
  const auto captureWithScreen = [&]() {
    const auto pixmap = screen->grabWindow(window->winId());
    if (pixmap.isNull()) {
      captureError = QStringLiteral("desktop screenshot capture returned null");
      return false;
    }
    return pixmap.save(path, "PNG") ? true : writeFailure();
  };
  const auto captureWithQuickWindow = [&]() {
    auto *quickWindow = qobject_cast<QQuickWindow *>(window);
    if (quickWindow == nullptr) {
      captureError = QStringLiteral("desktop window is not a quick window");
      return false;
    }
    const auto image = quickWindow->grabWindow();
    if (image.isNull()) {
      captureError =
          QStringLiteral("desktop quick screenshot capture returned null");
      return false;
    }
    return image.save(path, "PNG") ? true : writeFailure();
  };

  const auto platformName =
      qEnvironmentVariable("QT_QPA_PLATFORM").trimmed().toLower();
  auto preferQuickWindow =
      platformName == QStringLiteral("offscreen") ||
      platformName == QStringLiteral("minimal");
  auto allowScreenCapture = true;
#ifdef Q_OS_DARWIN
  // macOS screen capture can trigger the system screen-recording permission
  // prompt, which pollutes smoke screenshots. QQuickWindow capture stays inside
  // the app window and avoids that OS-level overlay.
  preferQuickWindow = true;
  allowScreenCapture = false;
#endif
  if (preferQuickWindow && captureWithQuickWindow()) {
    return true;
  }
  if (allowScreenCapture && captureWithScreen()) {
    return true;
  }
  if (!preferQuickWindow && captureWithQuickWindow()) {
    return true;
  }

  if (errorMessage != nullptr) {
    *errorMessage = captureError.isEmpty()
                        ? QStringLiteral("desktop screenshot capture failed")
                        : captureError;
  }
  return false;
}

void saveDesktopSmokeScreenshotAsync(
    QQmlApplicationEngine *engine, const QString &path, QObject *context,
    int timeoutMs, std::function<void(bool, const QString &)> completed) {
  QString errorMessage;
  if (!ensureDesktopSmokeScreenshotDirectory(path, &errorMessage)) {
    completed(false, errorMessage);
    return;
  }
  auto *rootItem = desktopSmokeRootQuickItem(engine);
  if (rootItem == nullptr) {
    completed(false, QStringLiteral("no QML root item to capture"));
    return;
  }
  auto grab = rootItem->grabToImage();
  if (grab.isNull()) {
    completed(false,
              QStringLiteral("desktop quick item capture did not start"));
    return;
  }
  auto finished = std::make_shared<bool>(false);
  QTimer::singleShot(timeoutMs, context, [finished, completed]() {
    if (*finished) {
      return;
    }
    *finished = true;
    completed(false,
              QStringLiteral("desktop quick item capture timed out"));
  });
  QObject::connect(grab.data(), &QQuickItemGrabResult::ready, context,
                   [grab, path, finished, completed]() {
                     if (*finished) {
                       return;
                     }
                     *finished = true;
                     const auto image = grab->image();
                     if (image.isNull()) {
                       completed(false,
                                 QStringLiteral(
                                     "desktop quick item capture returned null"));
                       return;
                     }
                     if (!image.save(path, "PNG")) {
                       completed(false,
                                 QStringLiteral(
                                     "failed to write desktop screenshot: %1")
                                     .arg(path));
                       return;
                     }
                     completed(true, QString());
                   });
}

void configureDesktopSmoke(QCoreApplication *app,
                           ChaftController *controller,
                           QQmlApplicationEngine *engine) {
  if (!desktopSmokeFlagEnabled()) {
    return;
  }

  const auto expectedText =
      qEnvironmentVariable("CHAFT_DESKTOP_SMOKE_EXPECT_TEXT").trimmed();
  const auto expectNoWorkspace = desktopSmokeExpectNoWorkspace();
  const auto expectReachable = desktopSmokeExpectReachable();
  const auto expectedReachabilityRoute =
      desktopSmokeExpectedReachabilityRoute();
  const auto screenshotPath =
      qEnvironmentVariable("CHAFT_DESKTOP_SMOKE_SCREENSHOT").trimmed();
  const auto screenshotDelayMs = desktopSmokeScreenshotDelayMs();
  const auto timeoutMs = desktopSmokeTimeoutMs();
  const auto completed = std::make_shared<bool>(false);

  const auto checkSnapshot = [app, controller, engine, expectedText,
                              screenshotPath, expectNoWorkspace, completed,
                              screenshotDelayMs, expectReachable,
                              expectedReachabilityRoute]() {
    if (*completed) {
      return;
    }
    const auto snapshot = controller->workspaceSnapshot();
    if (expectNoWorkspace) {
      if (!desktopSmokeReadyForEmptyRuntime(controller, snapshot)) {
        return;
      }
    } else if (!desktopSmokeSnapshotContainsText(snapshot, expectedText)) {
      return;
    }
    if (expectReachable && !desktopSmokeReachabilityMatches(
                               controller, expectedReachabilityRoute)) {
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

    QTimer::singleShot(screenshotDelayMs, app,
                       [engine, screenshotPath, workspaceId]() {
      const auto finishScreenshot = [screenshotPath, workspaceId](
                                        bool ok, const QString &errorMessage) {
        if (!ok) {
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
      };
      if (desktopSmokeQuickItemCaptureEnabled()) {
        saveDesktopSmokeScreenshotAsync(engine, screenshotPath,
                                        QCoreApplication::instance(),
                                        desktopSmokeScreenshotCaptureTimeoutMs(),
                                        finishScreenshot);
        return;
      }
      QString errorMessage;
      finishScreenshot(saveDesktopSmokeScreenshot(screenshotPath, &errorMessage),
                       errorMessage);
    });
  };

  QObject::connect(controller, &ChaftController::workspaceSnapshotChanged, app,
                   checkSnapshot, Qt::QueuedConnection);
  QObject::connect(controller, &ChaftController::syncStatusChanged, app,
                   checkSnapshot, Qt::QueuedConnection);
  QObject::connect(controller, &ChaftController::hostedPeerChanged, app,
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

void setLaunchEnvironmentValue(const char *name, const QString &value) {
  const auto normalized = value.trimmed();
  if (!normalized.isEmpty()) {
    qputenv(name, normalized.toUtf8());
  }
}

bool launchOptionValue(const QString &argument, const QString &option,
                       int *index, int argc, char *argv[],
                       QString *value) {
  const auto prefix = option + QStringLiteral("=");
  if (argument.startsWith(prefix)) {
    *value = argument.mid(prefix.size());
    return true;
  }
  if (argument == option && *index + 1 < argc) {
    ++(*index);
    *value = QString::fromLocal8Bit(argv[*index]);
    return true;
  }
  return false;
}

void applyDesktopLaunchEnvironment(int argc, char *argv[]) {
  for (int index = 1; index < argc; ++index) {
    const auto argument = QString::fromLocal8Bit(argv[index]);
    QString value;
    if (argument == QStringLiteral("--local-development-networking")) {
      qputenv("CHAFT_DESKTOP_ALLOW_LOOPBACK_FALLBACK", "1");
      qputenv("CHAFT_IROH_ALLOW_PUBLIC_RELAYS", "0");
      qputenv("CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY", "0");
    } else if (launchOptionValue(argument, QStringLiteral("--ffi-library"),
                                 &index, argc, argv, &value)) {
      setLaunchEnvironmentValue("CHAFT_FFI_LIBRARY", value);
    } else if (launchOptionValue(argument, QStringLiteral("--qml-import-root"),
                                 &index, argc, argv, &value)) {
      setLaunchEnvironmentValue("CHAFT_DESKTOP_QML_IMPORT_ROOT", value);
    } else if (launchOptionValue(argument, QStringLiteral("--runtime-dir"),
                                 &index, argc, argv, &value)) {
      setLaunchEnvironmentValue("CHAFT_RUNTIME_DIR", value);
    } else if (launchOptionValue(argument, QStringLiteral("--instance-label"),
                                 &index, argc, argv, &value)) {
      setLaunchEnvironmentValue("CHAFT_DESKTOP_INSTANCE_LABEL", value);
    } else if (launchOptionValue(argument, QStringLiteral("--workspace-id"),
                                 &index, argc, argv, &value)) {
      setLaunchEnvironmentValue("CHAFT_WORKSPACE_ID", value);
    } else if (launchOptionValue(argument, QStringLiteral("--identity-file"),
                                 &index, argc, argv, &value)) {
      setLaunchEnvironmentValue("CHAFT_IDENTITY_FILE", value);
    } else if (launchOptionValue(argument, QStringLiteral("--peer-endpoint"),
                                 &index, argc, argv, &value)) {
      setLaunchEnvironmentValue("CHAFT_PEER_ENDPOINT", value);
    }
  }
}

namespace {

void loadBundledDesktopFonts() {
  const QStringList fontResources = {
      QStringLiteral(":/fonts/SpaceGrotesk-Regular.ttf"),
      QStringLiteral(":/fonts/SpaceGrotesk-Medium.ttf"),
      QStringLiteral(":/fonts/SpaceGrotesk-Bold.ttf"),
      QStringLiteral(":/fonts/JetBrainsMono-Regular.ttf"),
      QStringLiteral(":/fonts/JetBrainsMono-Medium.ttf"),
      QStringLiteral(":/fonts/JetBrainsMono-SemiBold.ttf"),
  };
  for (const auto &fontResource : fontResources) {
    if (QFontDatabase::addApplicationFont(fontResource) < 0) {
      qWarning("chaft desktop could not load bundled font %s",
               qPrintable(fontResource));
    }
  }
  // Missing bundled families fall back to the platform font instead of
  // blocking startup.
  if (QFontDatabase::hasFamily(QStringLiteral("Space Grotesk"))) {
    QGuiApplication::setFont(QFont(QStringLiteral("Space Grotesk")));
  }
}

} // namespace

int main(int argc, char *argv[]) {
  applyDesktopLaunchEnvironment(argc, argv);
  applyDesktopReachabilityDefaults();

  if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE")) {
    qputenv("QT_QUICK_CONTROLS_STYLE", "ChaftStyle");
  }
  if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_FALLBACK_STYLE")) {
    qputenv("QT_QUICK_CONTROLS_FALLBACK_STYLE", "Basic");
  }

  QApplication app(argc, argv);
  std::unique_ptr<QLockFile> runtimeLock;
  const auto runtimeDir = defaultRuntimeDir();
  if (!runtimeDir.isEmpty()) {
    if (!QDir().mkpath(runtimeDir)) {
      std::fprintf(stderr, "chaft desktop could not create runtime directory: %s\n",
                   qPrintable(runtimeDir));
      return 73;
    }
    runtimeLock = std::make_unique<QLockFile>(
        QDir(runtimeDir).filePath(QStringLiteral("desktop-runtime.lock")));
    if (!runtimeLock->tryLock(0)) {
      qint64 holderPid = 0;
      QString holderHost;
      QString holderApplication;
      runtimeLock->getLockInfo(&holderPid, &holderHost, &holderApplication);
      std::fprintf(
          stderr,
          "chaft desktop runtime is already open: %s (pid=%lld host=%s app=%s)\n",
          qPrintable(runtimeDir), static_cast<long long>(holderPid),
          qPrintable(holderHost), qPrintable(holderApplication));
      return 73;
    }
  }
  loadBundledDesktopFonts();

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
  configureDesktopSmoke(&app, &chaftController, &engine);

  return app.exec();
}

#include "main.moc"
