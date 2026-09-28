using EdgeOptimizer.Settings.Core.Models;

namespace EdgeOptimizer.Settings.Core.Tests;

public sealed class NotificationTests
{
    [Fact]
    public void ProcessItemNotifiesOnlyWhenSelectionChanges()
    {
        // Verifies repeated assignment of an unchanged value does not produce redundant UI notifications.
        var item = new ProcessItem("test.exe", "0%", "1 MB", false);
        var notifications = 0;
        item.PropertyChanged += (_, _) => notifications++;

        item.IsSelected = false;
        item.IsSelected = true;
        item.IsSelected = true;

        Assert.Equal(1, notifications);
    }
}
