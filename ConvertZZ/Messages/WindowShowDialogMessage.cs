using System.Windows;
using CommunityToolkit.Mvvm.Messaging.Messages;

namespace ConvertZZ.Core.Messages
{
    public class WindowShowDialogMessage : RequestMessage<bool?>
    {
        public WindowShowDialogMessage(Window window)
        {
            Window = window;
        }

        public Window Window { get; }
    }
}