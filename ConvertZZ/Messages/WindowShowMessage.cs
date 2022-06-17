using System.Windows;
using CommunityToolkit.Mvvm.Messaging.Messages;

namespace ConvertZZ.Core.Messages
{
    public class WindowShowMessage : RequestMessage<bool?>
    {
        public WindowShowMessage(Window window)
        {
            Window = window;
        }

        public Window Window { get; }
    }
}