using CommunityToolkit.Mvvm.Messaging.Messages;

namespace ConvertZZ.Core.Messages
{
    //public class InvokeCommandMessage : AsyncRequestMessage<object?>
    //{
    //    public InvokeCommandMessage(object? parameter = null)
    //    {
    //        Parameter = parameter;
    //    }

    //    public object? Parameter { get; }
    //}

    public class AsyncInvokeCommandMessage<TOutput> : AsyncRequestMessage<TOutput>
    {
        public AsyncInvokeCommandMessage()
        {
        }
    }

    public class AsyncInvokeCommandMessage<TInput, TOutput> : AsyncRequestMessage<TOutput>
    {
        public AsyncInvokeCommandMessage(TInput? parameter)
        {
            Parameter = parameter;
        }

        public TInput? Parameter { get; }
    }
}