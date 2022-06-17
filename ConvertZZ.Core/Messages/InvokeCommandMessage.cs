using CommunityToolkit.Mvvm.Messaging.Messages;

namespace ConvertZZ.Core.Messages
{
    public class InvokeCommandMessage : AsyncRequestMessage<object?>
    {
        public InvokeCommandMessage(object? parameter = null)
        {
            Parameter = parameter;
        }

        public object? Parameter { get; }
    }

    public class InvokeCommandMessage<T> : AsyncRequestMessage<T>
    {
        public InvokeCommandMessage(object? parameter = null)
        {
            Parameter = parameter;
        }

        public object? Parameter { get; }
    }

    public class AsyncInvokeCommandMessage<T> : AsyncRequestMessage<T>
    {
        public AsyncInvokeCommandMessage(object? parameter = null)
        {
            Parameter = parameter;
        }

        public object? Parameter { get; }
    }
}