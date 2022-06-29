using System.Windows.Input;
using CommunityToolkit.Mvvm.Input;
using CommunityToolkit.Mvvm.Messaging;
using ConvertZZ.Core.Messages;

namespace ConvertZZ.Core.Helpers
{
    public partial class MessengerHelper
    {
        public static ICommand RegistInvokeCommandMessageAndReturnCommand<TRecipient>(TRecipient recipient, Action action, EnumCommand enumCommand) where TRecipient : class
        {
            RelayCommand relayCommand = new(action);
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);

            if (WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<object?>, EnumAdapter<EnumCommand>>(recipient, adapter) == false)
            {
                WeakReferenceMessenger.Default.Register<AsyncInvokeCommandMessage<object?>, EnumAdapter<EnumCommand>>(recipient, adapter, (r, m) =>
                {
                    relayCommand.Execute(null);
                    m.Reply(-1);
                });
            }
            return relayCommand;
        }

        public static void RegistInvokeCommandMessage<TRecipient, TInput, TOutput>(TRecipient recipient, Func<TInput?, Task<TOutput>> action, EnumCommand enumCommand) where TRecipient : class
        {
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);

            if (WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<TInput, TOutput>, EnumAdapter<EnumCommand>>(recipient, adapter) == false)
            {
                WeakReferenceMessenger.Default.Register<AsyncInvokeCommandMessage<TInput, TOutput>, EnumAdapter<EnumCommand>>(recipient, adapter, async (r, m) =>
                {
                    var output = await action(m.Parameter);
                    m.Reply(output);
                });
            }
            return;
        }

        public static void RegistInvokeCommandMessage<TRecipient, TInput, TOutput>(TRecipient recipient, Func<TInput?, TOutput> action, EnumCommand enumCommand) where TRecipient : class
        {
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);

            if (WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<TInput, TOutput>, EnumAdapter<EnumCommand>>(recipient, adapter) == false)
            {
                WeakReferenceMessenger.Default.Register<AsyncInvokeCommandMessage<TInput, TOutput>, EnumAdapter<EnumCommand>>(recipient, adapter, (r, m) =>
                {
                    var output = action(m.Parameter);
                    m.Reply(output);
                });
            }
            return;
        }

        public static void RegistInvokeCommandMessage<TRecipient, TOutput>(TRecipient recipient, Func<Task<TOutput>> action, EnumCommand enumCommand) where TRecipient : class
        {
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);

            if (WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<TOutput>, EnumAdapter<EnumCommand>>(recipient, adapter) == false)
            {
                WeakReferenceMessenger.Default.Register<AsyncInvokeCommandMessage<TOutput>, EnumAdapter<EnumCommand>>(recipient, adapter, async (r, m) =>
                {
                    var output = await action();
                    m.Reply(output);
                });
            }
            return;
        }

        public static void RegistInvokeCommandMessage<TRecipient, TOutput>(TRecipient recipient, Func<TOutput> action, EnumCommand enumCommand) where TRecipient : class
        {
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);

            if (WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<TOutput>, EnumAdapter<EnumCommand>>(recipient, adapter) == false)
            {
                WeakReferenceMessenger.Default.Register<AsyncInvokeCommandMessage<TOutput>, EnumAdapter<EnumCommand>>(recipient, adapter, (r, m) =>
                {
                    var output = action();
                    m.Reply(output);
                });
            }
            return;
        }

        public static TOutput Send<TOutput>(EnumCommand enumCommand)
        {
            return SendAsync<TOutput>(enumCommand).Result;
        }

        public static TOutput Send<TInput, TOutput>(EnumCommand enumCommand, TInput? parameter)
        {
            return SendAsync<TInput, TOutput>(enumCommand, parameter).Result;
        }

        public static async Task<TOutput> SendAsync<TOutput>(EnumCommand enumCommand)
        {
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);
            return await WeakReferenceMessenger.Default.Send(new AsyncInvokeCommandMessage<TOutput>(), adapter);
        }

        public static async Task<TOutput> SendAsync<TInput, TOutput>(EnumCommand enumCommand, TInput? parameter)
        {
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);
            return await WeakReferenceMessenger.Default.Send(new AsyncInvokeCommandMessage<TInput, TOutput>(parameter), adapter);
        }

        public static void Send(EnumCommand enumCommand)
        {
            Send<object?>(enumCommand);
        }

        public static void Send<TInput>(EnumCommand enumCommand, TInput? parameter)
        {
            Send<TInput, object?>(enumCommand, parameter);
        }

        public static async Task SendAsync(EnumCommand enumCommand)
        {
            await SendAsync<object?>(enumCommand);
        }

        public static async Task SendAsync<TInput>(EnumCommand enumCommand, TInput? parameter)
        {
            await SendAsync<TInput, object?>(enumCommand, parameter);
        }
    }
}