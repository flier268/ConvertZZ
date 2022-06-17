namespace ConvertZZ.Core.Helpers
{
    public struct EnumAdapter<T> : IEquatable<EnumAdapter<T>> where T : Enum
    {
        private T Command { get; }

        public EnumAdapter(T myEnum)
        {
            Command = myEnum;
        }

        public bool Equals(EnumAdapter<T> other)
        {
            return Command.Equals(other.Command);
        }
    }
}