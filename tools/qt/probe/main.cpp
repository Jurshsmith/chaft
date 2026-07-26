#include <QNetworkRequest>
#include <QQmlEngine>
#include <QQuickItem>
#include <QWidget>

int main()
{
    QNetworkRequest request;
    QQmlEngine engine;
    QQuickItem item;
    QWidget *widget = nullptr;
    return request.url().isEmpty() && engine.rootContext() && !item.isVisible()
            && widget == nullptr
        ? 0
        : 1;
}
